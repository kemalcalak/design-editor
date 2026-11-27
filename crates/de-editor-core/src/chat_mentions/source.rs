//! Where the `@` popup's file list comes from (#398). Core only defines the
//! seam: listing a folder is disk I/O, which the host does off the UI
//! thread (`de_host_services::chat_mention_scan::ProjectFileScanner`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One listing of a project folder for the `@` popup.
#[derive(Debug, Clone)]
pub struct ProjectFileList {
    /// The folder that was listed.
    pub root: PathBuf,
    /// Project-relative paths with `/` separators, sorted.
    pub files: Arc<[String]>,
    /// Raise the "only part of this folder is offered" notice (the listing
    /// was cut, and the user has not been told for this folder yet).
    pub warn: bool,
}

/// A host's folder lister. Nothing behind it may touch the disk on the UI
/// thread.
pub trait ProjectFileSource {
    /// Ask for a current listing of `root`. Answers from the cache at once
    /// when there is one; a stale or missing cache starts a background
    /// listing that [`ProjectFileSource::poll`] hands over later.
    fn request(&mut self, root: &Path) -> Option<ProjectFileList>;
    /// A listing that finished since the last call, if any.
    fn poll(&mut self) -> Option<ProjectFileList>;
}
