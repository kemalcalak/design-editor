//! Where the popup's file and skill lists come from (#398). Core only
//! defines the seams: listing a folder is disk I/O, which the host does off
//! the UI thread (`de_host_services::chat_mention_scan::ProjectFileScanner`,
//! `de_host_services::chat_skill_context::ChatSkillScanner`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::SkillOption;

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

/// One listing of the skill folders for a project (or none).
#[derive(Debug, Clone)]
pub struct SkillList {
    /// The project folder the list was made for.
    pub root: Option<PathBuf>,
    pub skills: Arc<[SkillOption]>,
}

/// A host's skill lister. Nothing behind it may touch the disk on the UI
/// thread.
pub trait SkillListSource {
    /// Ask for a current listing for `root`; answers from the cache at once
    /// when there is one, else [`SkillListSource::poll`] hands it over
    /// later.
    fn request(&mut self, root: Option<&Path>) -> Option<SkillList>;
    /// A listing that finished since the last call, if any.
    fn poll(&mut self) -> Option<SkillList>;
}
