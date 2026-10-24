//! Linked component libraries (#15).
//!
//! A document can link other `.de` (or pen.dev `.pen`) files as component
//! libraries. The link itself is a path relative to the document's folder;
//! it is persisted in the file's `editorMeta.libraries` so the schema stays
//! untouched.
//!
//! The library's reusable masters are cached on the document's hidden
//! `Components` page with their ids kept verbatim, so instances (`ref`
//! nodes) resolve through the ordinary cross-page resolver and keep
//! rendering when the library file goes missing. [`LibraryLink::masters`]
//! records which cached nodes the library owns: a re-sync replaces exactly
//! those, and nothing the user authored locally.
//!
//! This module holds the data types only; the sync itself is
//! `library_sync.rs` (pure, no file IO). Reading files is the loader's job
//! (`de_pen_loader::library_files`), so this crate stays wasm32-clean.

use std::collections::BTreeMap;
use std::sync::Arc;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableDefinition;

/// One library the document links to, as persisted in `editorMeta`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLink {
    /// The library file, relative to the document's folder (absolute only
    /// while the document has never been saved).
    pub path: String,
    /// Short name (pen.dev's `imports` key). Optional: `display_name` falls
    /// back to the file stem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Ids of the masters this library put on the `Components` page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masters: Vec<String>,
    /// Names of the variables this library put into the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<String>,
}

impl LibraryLink {
    /// A fresh link to `path` with nothing synced yet.
    pub fn new(path: impl Into<String>, alias: Option<String>) -> Self {
        Self {
            path: path.into(),
            alias,
            ..Self::default()
        }
    }

    /// Name the panel and MCP show: the alias, else the file stem.
    pub fn display_name(&self) -> String {
        if let Some(alias) = self.alias.as_deref().filter(|a| !a.trim().is_empty()) {
            return alias.to_string();
        }
        let file = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        let stem = file
            .strip_suffix(".lib.de")
            .or_else(|| file.strip_suffix(".de"))
            .or_else(|| file.strip_suffix(".pen"))
            .unwrap_or(file);
        stem.to_string()
    }
}

/// How the last attempt to read a linked library went. Runtime only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LibraryStatus {
    /// Not read in this session yet (cached masters still render).
    #[default]
    NotLoaded,
    /// Read and synced.
    Synced {
        /// Masters the library provides now.
        components: usize,
        /// Library masters skipped because a local node uses the same id.
        conflicts: usize,
    },
    /// The file does not exist (or this host cannot read files).
    Missing,
    /// The file exists but could not be read or parsed.
    Invalid(String),
}

impl LibraryStatus {
    /// Whether the panel should flag this library.
    pub fn is_problem(&self) -> bool {
        matches!(self, Self::Missing | Self::Invalid(_))
    }

    /// Stable lower-case word for MCP results.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotLoaded => "not-loaded",
            Self::Synced { .. } => "synced",
            Self::Missing => "missing",
            Self::Invalid(_) => "invalid",
        }
    }
}

/// What a library file provides, already harvested by the loader.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryContent {
    /// Resolved reusable master roots, ids verbatim.
    pub masters: Vec<PenNode>,
    /// The library's variable definitions.
    pub variables: BTreeMap<String, VariableDefinition>,
    /// The library's theme axes.
    pub themes: BTreeMap<String, Vec<String>>,
}

/// A link plus its runtime state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LinkedLibrary {
    pub link: LibraryLink,
    pub status: LibraryStatus,
    /// Last content read from disk. Undo / redo re-apply it so history
    /// cannot roll the cached masters back to an older library version.
    pub content: Option<Arc<LibraryContent>>,
}

/// What the Libraries section asked the host to do (needs file access).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryRequest {
    /// Pick a library file and link it.
    Link,
    /// Re-read every linked library from disk.
    Reload,
}

/// A press in the Libraries section, routed by both hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryPanelAction {
    Link,
    Reload,
    /// Remove the link at this path.
    Unlink(String),
}

/// All library state of the open document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryLinks {
    pub entries: Vec<LinkedLibrary>,
    /// Folder of the open document (relative links resolve against it).
    /// `None` for an unsaved document or a host without files.
    pub base_dir: Option<String>,
    /// Queued host request from the panel.
    pub request: Option<LibraryRequest>,
}

impl LibraryLinks {
    /// Index of the link stored under `path`.
    pub fn position(&self, path: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.link.path == path)
    }

    /// The persisted links, in order.
    pub fn links(&self) -> Vec<LibraryLink> {
        self.entries.iter().map(|e| e.link.clone()).collect()
    }

    /// Replace the links with `links` (from a file), statuses reset.
    pub fn set_links(&mut self, links: Vec<LibraryLink>) {
        self.entries = links
            .into_iter()
            .map(|link| LinkedLibrary {
                link,
                ..LinkedLibrary::default()
            })
            .collect();
    }
}

/// Library edits that ride the command path (MCP, panel, hosts).
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryCommand {
    /// Link (or re-sync) `path` with freshly read content.
    Sync {
        path: String,
        alias: Option<String>,
        content: Arc<LibraryContent>,
    },
    /// Record that `path` could not be read; cached masters stay.
    Unavailable { path: String, status: LibraryStatus },
    /// Remove the link at `path`.
    Unlink { path: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_prefers_alias_then_stem() {
        assert_eq!(
            LibraryLink::new("kits/ui.lib.de", None).display_name(),
            "ui"
        );
        assert_eq!(
            LibraryLink::new("a\\b\\Brand.de", None).display_name(),
            "Brand"
        );
        assert_eq!(
            LibraryLink::new("x.pen", Some("kit".into())).display_name(),
            "kit"
        );
    }

    #[test]
    fn link_round_trips_and_omits_empty_fields() {
        let link = LibraryLink::new("ui.de", None);
        let json = serde_json::to_string(&link).unwrap();
        assert_eq!(json, r#"{"path":"ui.de"}"#);
        let back: LibraryLink = serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(back, link);
    }
}
