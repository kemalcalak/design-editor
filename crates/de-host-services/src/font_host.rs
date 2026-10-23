//! The seam between the MCP font tools and the host that owns fonts.
//!
//! Downloading, persisting and registering a family needs the desktop
//! `FontStore` and the native font manager, neither of which lives in this
//! crate. The desktop binary installs a [`FontHost`] at startup (the same
//! shape as `home_preview::set_written_notifier`); `load_fonts` and the
//! `fontIssues` field of `get_design_quality` ask it. A host that installs
//! none (the thin web server) reports font downloads as unavailable.

use std::sync::OnceLock;
use std::time::Duration;

use de_editor_core::EditorState;
use serde::Serialize;

/// The outcome of making one family available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontLoadResult {
    /// The family as requested.
    pub family: String,
    /// `available` (installed, bundled or imported already), `cached`
    /// (downloaded on an earlier run), `downloaded`, `downloading` (still
    /// running when the wait ended; it finishes in the background),
    /// `notOnGoogle`, or `failed`.
    pub status: &'static str,
    /// Faces stored by a fresh download.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faces: Option<usize>,
    /// Why a download failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A family the document names that paints with a fallback face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontIssue {
    pub family: String,
    /// Text runs that use the family.
    pub run_count: u32,
    /// `missing`, `downloading`, `notOnGoogle`, or `failed`.
    pub status: &'static str,
    /// What the agent can do about it.
    pub hint: String,
}

/// Font operations the MCP tools delegate to the installing host.
pub trait FontHost: Send + Sync {
    /// Make each family available, downloading from Google Fonts what this
    /// machine lacks. Waits at most `wait`; downloads still running then
    /// are reported `downloading` and finish in the background.
    fn load(&self, families: &[String], wait: Duration) -> Vec<FontLoadResult>;

    /// The document's families that render with a fallback face.
    fn font_issues(&self, state: &EditorState) -> Vec<FontIssue>;
}

static HOST: OnceLock<Box<dyn FontHost>> = OnceLock::new();

/// Install the process's font host. First call wins.
pub fn install(host: Box<dyn FontHost>) {
    let _ = HOST.set(host);
}

/// The installed font host, if any.
pub fn installed() -> Option<&'static dyn FontHost> {
    HOST.get().map(|host| host.as_ref())
}
