//! Linked component library tools (#15): `list_libraries`, `add_library`,
//! `remove_library`.
//!
//! A linked library is another `.de` (or pen.dev `.pen`) file whose
//! reusable components the document uses live: its masters are cached on
//! the hidden `Components` page and re-read whenever the document opens or
//! (on desktop) the file changes. Instances are ordinary `ref` nodes that
//! point at a library master's id.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use de_editor_core::{EditorState, LibraryCommand, LibraryLink, LibraryStatus};
use serde_json::{json, Value};

use super::{EditorCommand, McpTool, ToolErrorCode, ToolOutcome};

/// One link as the tools see it.
#[derive(Clone)]
struct LinkRow {
    link: LibraryLink,
    status: LibraryStatus,
}

/// Snapshot shared by the three tools.
pub struct LibraryTool {
    name: &'static str,
    base_dir: Option<PathBuf>,
    rows: Vec<LinkRow>,
}

fn snapshot(name: &'static str, state: &EditorState) -> LibraryTool {
    LibraryTool {
        name,
        base_dir: state
            .editor_ui
            .libraries
            .base_dir
            .clone()
            .map(PathBuf::from),
        rows: state
            .editor_ui
            .libraries
            .entries
            .iter()
            .map(|entry| LinkRow {
                link: entry.link.clone(),
                status: entry.status.clone(),
            })
            .collect(),
    }
}

pub fn list_libraries_snapshot(state: &EditorState) -> LibraryTool {
    snapshot("list_libraries", state)
}

pub fn add_library_snapshot(state: &EditorState) -> LibraryTool {
    snapshot("add_library", state)
}

pub fn remove_library_snapshot(state: &EditorState) -> LibraryTool {
    snapshot("remove_library", state)
}

impl McpTool for LibraryTool {
    fn name(&self) -> &str {
        self.name
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        match self.name {
            "add_library" => self.add(args),
            "remove_library" => self.remove(args),
            // The only other name the snapshots are built with.
            "list_libraries" => self.list(),
            other => ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("unknown library tool {other}"),
            ),
        }
    }
}

impl LibraryTool {
    fn list(&self) -> ToolOutcome {
        let libraries: Vec<Value> = self.rows.iter().map(|row| self.row_json(row)).collect();
        ToolOutcome::OkJson(json!({ "count": libraries.len(), "libraries": libraries }).to_string())
    }

    fn row_json(&self, row: &LinkRow) -> Value {
        let file = de_pen_loader::resolve_library_path(self.base_dir.as_deref(), &row.link.path);
        let mut out = json!({
            "path": row.link.path,
            "name": row.link.display_name(),
            "file": file.to_string_lossy(),
            "status": row.status.as_str(),
            "componentIds": row.link.masters,
        });
        if let Some(alias) = &row.link.alias {
            out["alias"] = json!(alias);
        }
        match &row.status {
            LibraryStatus::Synced {
                components,
                conflicts,
            } => {
                out["components"] = json!(components);
                out["conflicts"] = json!(conflicts);
            }
            LibraryStatus::Invalid(error) => out["error"] = json!(error),
            LibraryStatus::Missing | LibraryStatus::NotLoaded => {}
        }
        out
    }

    fn add(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(raw) = arg(args, &["path"]) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "path is required".into());
        };
        let file = self.resolve_input(raw);
        let path = de_pen_loader::library_link_path(self.base_dir.as_deref(), &file);
        let alias = arg(args, &["alias"]).map(str::to_string);
        // The same read the panel's Link runs, so both create the same link.
        let command = de_pen_loader::library_command(self.base_dir.as_deref(), &path, alias);
        let content = match &command {
            LibraryCommand::Sync { content, .. } => Arc::clone(content),
            LibraryCommand::Unavailable { status, .. } => {
                let reason = match status {
                    LibraryStatus::Invalid(error) => error.clone(),
                    _ => "library file not found".to_string(),
                };
                return ToolOutcome::Err(
                    ToolErrorCode::ToolFailed,
                    format!("{}: {reason}", file.display()),
                );
            }
            LibraryCommand::Unlink { .. } => unreachable!("library_command never unlinks"),
        };
        let ids: Vec<&str> = content
            .masters
            .iter()
            .map(de_editor_core::PenNodeExt::id_str)
            .collect();
        let result = json!({
            "path": path,
            "file": file.to_string_lossy(),
            "libraryComponents": ids.len(),
            "libraryComponentIds": ids,
            "note": "Instances are `ref` nodes pointing at a component id. \
                     A component whose id the document already uses is skipped; \
                     list_libraries shows the linked ones.",
        });
        ToolOutcome::OkJsonWithCommand(result.to_string(), EditorCommand::Library(command))
    }

    fn remove(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let Some(raw) = arg(args, &["path"]) else {
            return ToolOutcome::Err(ToolErrorCode::MissingArgument, "path is required".into());
        };
        let file = self.resolve_input(raw);
        let found = self.rows.iter().find(|row| {
            row.link.path == raw
                || row.link.display_name() == raw
                || de_pen_loader::resolve_library_path(self.base_dir.as_deref(), &row.link.path)
                    == file
        });
        let Some(row) = found else {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("no linked library matches {raw:?}; see list_libraries"),
            );
        };
        let mut out = BTreeMap::new();
        out.insert("removed".into(), row.link.path.clone());
        ToolOutcome::OkWithCommand(
            out,
            EditorCommand::Library(LibraryCommand::Unlink {
                path: row.link.path.clone(),
            }),
        )
    }

    /// A user-supplied path: relative to the document folder when there is
    /// one, else to the working directory.
    fn resolve_input(&self, raw: &str) -> PathBuf {
        match &self.base_dir {
            Some(base) => de_pen_loader::resolve_library_path(Some(base), raw),
            None => PathBuf::from(de_pen_loader::absolute_library_path(Path::new(raw))),
        }
    }
}

fn arg<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| args.get(*key))
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "library_tools_tests.rs"]
mod tests;
