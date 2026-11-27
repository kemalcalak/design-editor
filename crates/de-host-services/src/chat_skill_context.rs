//! User skills in the chat (#398, aligned with #338): where they are read
//! from, the `/` popup's list, and a picked skill's instructions for the
//! turn.
//!
//! Folders, in precedence order (`de_ai_skills::user_skills`):
//!
//! 1. `<document folder>/.design-editor/skills/` — the project's skills,
//! 2. `<config dir>/skills/` — the user's own (`de_config_store`).
//!
//! Listing reads only front matter and runs on a worker thread
//! ([`ChatSkillScanner`]): the folders are listed again when the `/` popup
//! opens or the project folder changes, so a skill dropped into a folder is
//! picked up without a restart. The listing also reads the skills named like
//! a built-in skill; the scanner installs them as the built-ins'
//! replacements (`de_ai_skills::loader::set_skill_overrides`) on the UI
//! side, and only for the folder still open, so a late listing for a folder
//! left behind never replaces another project's. A picked skill's
//! instructions are read from its file on a worker when it is staged
//! (`chat_mention_prefetch`), checked again (still inside its folder, still
//! the same name, within the size limit).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use de_ai_skills::loader::{set_skill_overrides, SkillEntry};
use de_ai_skills::user_skills::{load_user_skills, read_skill_body, SkillSource, UserSkillSet};
use de_editor_core::chat_mentions::{SkillList, SkillListSource, SkillOption, SkillOrigin};

/// The project skills folder, relative to the document's folder.
pub const PROJECT_SKILLS_DIR: &str = ".design-editor/skills";
/// The user skills folder, inside the app's config directory.
pub const USER_SKILLS_DIR: &str = "skills";
/// Characters of one skill's instructions handed to the model in a turn.
pub const MAX_SKILL_CONTEXT_CHARS: usize = 12_000;
/// Characters of all picked skills' instructions in one turn together.
pub const MAX_TURN_SKILL_CHARS: usize = 24_000;
/// A listing younger than this is served without listing again.
const FRESH_FOR: Duration = Duration::from_secs(2);

/// The folders skills are read from, project first. The user folder is left
/// out when the config directory cannot be resolved.
pub fn skill_folders(project_root: Option<&Path>) -> Vec<(SkillSource, PathBuf)> {
    let mut folders = Vec::with_capacity(2);
    if let Some(root) = project_root {
        folders.push((SkillSource::Project, root.join(PROJECT_SKILLS_DIR)));
    }
    if let Ok(config) = de_config_store::design_editor_dir() {
        folders.push((SkillSource::User, config.join(USER_SKILLS_DIR)));
    }
    folders
}

/// List the chat's skills in `folders` (front matter only), marking those
/// that replace a built-in skill. Broken files are logged and left out (see
/// `UserSkillSet::issues`); a settings page for them is #338.
pub fn load_chat_skills(folders: &[(SkillSource, PathBuf)]) -> UserSkillSet {
    let builtin: Vec<&str> = de_ai_skills::loader::get_skill_registry()
        .iter()
        .map(|entry| entry.meta.name.as_str())
        .collect();
    let set = load_user_skills(folders, &builtin);
    for issue in &set.issues {
        eprintln!(
            "design-editor: skipped skill {}: {}",
            issue.path.display(),
            issue.error
        );
    }
    set
}

/// The `/` popup's rows for a listed set.
pub fn skill_options(set: &UserSkillSet) -> Vec<SkillOption> {
    set.skills
        .iter()
        .map(|skill| SkillOption {
            name: skill.meta.name.clone(),
            description: skill.meta.description.clone(),
            origin: match skill.source {
                SkillSource::Project => SkillOrigin::Project,
                SkillSource::User => SkillOrigin::User,
            },
            path: skill.path.to_string_lossy().into_owned(),
            folder: skill.folder.to_string_lossy().into_owned(),
            shadows_user: skill.shadows_user,
            overrides_builtin: skill.overrides_builtin,
        })
        .collect()
}

/// The skills of `set` that replace a built-in skill, with their
/// instructions read. One without its own `phase` keeps the built-in's
/// phases (an empty `phase`, see `resolve_skills_with_overrides`). One that
/// cannot be read is logged and left out, so the built-in stays.
pub fn builtin_overrides(set: &UserSkillSet) -> Vec<SkillEntry> {
    set.skills
        .iter()
        .filter(|skill| skill.overrides_builtin)
        .filter_map(
            |skill| match read_skill_body(&skill.path, &skill.folder, &skill.meta.name) {
                Ok(content) => {
                    let mut meta = skill.meta.clone();
                    if !skill.declares_phase {
                        meta.phase.clear();
                    }
                    Some(SkillEntry { meta, content })
                }
                Err(err) => {
                    eprintln!(
                        "design-editor: skipped skill {}: {err}",
                        skill.path.display()
                    );
                    None
                }
            },
        )
        .collect()
}

/// List the skills in `folders` for `project_root`, with the built-in
/// overrides they carry (not installed). Disk I/O: runs on the scanner's
/// worker thread.
pub fn list_chat_skills(
    project_root: Option<&Path>,
    folders: &[(SkillSource, PathBuf)],
) -> (SkillList, Vec<SkillEntry>) {
    let set = load_chat_skills(folders);
    let overrides = builtin_overrides(&set);
    let list = SkillList {
        root: project_root.map(Path::to_path_buf),
        skills: Arc::from(skill_options(&set)),
    };
    (list, overrides)
}

/// A picked skill's text for the context block: its description and its
/// instructions read from its file, cut at [`MAX_SKILL_CONTEXT_CHARS`].
/// `None` when the file is gone, left its folder, was renamed or is no
/// longer a valid skill. Disk I/O: hosts call it on a worker.
pub fn skill_instructions(skill: &SkillOption) -> Option<String> {
    let body = match read_skill_body(
        Path::new(&skill.path),
        Path::new(&skill.folder),
        &skill.name,
    ) {
        Ok(body) => body,
        Err(err) => {
            eprintln!("design-editor: skill {} unreadable: {err}", skill.path);
            return None;
        }
    };
    let body = body.trim();
    let mut text: String = body.chars().take(MAX_SKILL_CONTEXT_CHARS).collect();
    if body.chars().nth(MAX_SKILL_CONTEXT_CHARS).is_some() {
        text.push_str("\n[…skill cut at the context limit]");
    }
    Some(format!("{}\n{text}", skill.description.trim()))
}

/// Which folders a project's skills are listed from (injected in tests).
type FoldersFor = Arc<dyn Fn(Option<&Path>) -> Vec<(SkillSource, PathBuf)> + Send + Sync>;
type Listing = (SkillList, Vec<SkillEntry>);

struct Cached {
    list: SkillList,
    at: Instant,
}

/// Background, cached [`SkillListSource`] for the desktop host.
pub struct ChatSkillScanner {
    folders_for: FoldersFor,
    /// Install the overrides into the process-wide skill registry (off in
    /// tests, which read [`ChatSkillScanner::overrides`] instead).
    install: bool,
    cache: Option<Cached>,
    inflight: Option<(Option<PathBuf>, Receiver<Listing>)>,
    /// The project folder last asked for; only its listing is installed.
    wanted: Option<Option<PathBuf>>,
    /// The overrides installed from the last listing that landed.
    overrides: Vec<SkillEntry>,
}

impl Default for ChatSkillScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatSkillScanner {
    /// The desktop's scanner: the project and config skills folders, with
    /// overrides installed process-wide.
    pub fn new() -> Self {
        Self {
            folders_for: Arc::new(skill_folders),
            install: true,
            cache: None,
            inflight: None,
            wanted: None,
            overrides: Vec::new(),
        }
    }

    /// A scanner over the folders `folders_for` names that keeps its
    /// overrides to itself (tests).
    pub fn with_folders(
        folders_for: impl Fn(Option<&Path>) -> Vec<(SkillSource, PathBuf)> + Send + Sync + 'static,
    ) -> Self {
        Self {
            folders_for: Arc::new(folders_for),
            install: false,
            ..Self::new()
        }
    }

    /// Whether a listing is running on its worker (the host keeps waking
    /// to collect it).
    pub fn is_pending(&self) -> bool {
        self.inflight.is_some()
    }

    /// The overrides of built-in skills from the last listing installed.
    pub fn overrides(&self) -> &[SkillEntry] {
        &self.overrides
    }

    fn spawn(&mut self, root: Option<&Path>) {
        let (tx, rx) = mpsc::channel();
        let dir = root.map(Path::to_path_buf);
        let folders_for = Arc::clone(&self.folders_for);
        let spawned = thread::Builder::new()
            .name("de-skill-scan".into())
            .spawn(move || {
                let folders = folders_for(dir.as_deref());
                // The receiver may be gone (another folder was opened); the
                // listing is then simply dropped.
                let _ = tx.send(list_chat_skills(dir.as_deref(), &folders));
            });
        match spawned {
            Ok(_) => self.inflight = Some((root.map(Path::to_path_buf), rx)),
            Err(err) => eprintln!("design-editor: spawn skill scan failed: {err}"),
        }
    }

    /// Take a finished listing: installed and handed on only when it is for
    /// the folder last asked for, so a stale one never wins.
    fn land(&mut self, list: SkillList, overrides: Vec<SkillEntry>) -> Option<SkillList> {
        if self.wanted.as_ref() != Some(&list.root) {
            return None;
        }
        if self.install {
            set_skill_overrides(overrides.clone());
        }
        self.overrides = overrides;
        self.cache = Some(Cached {
            list: list.clone(),
            at: Instant::now(),
        });
        Some(list)
    }
}

impl SkillListSource for ChatSkillScanner {
    fn request(&mut self, root: Option<&Path>) -> Option<SkillList> {
        if self.wanted.as_ref().map(Option::as_deref) != Some(root) {
            self.wanted = Some(root.map(Path::to_path_buf));
        }
        let cached = self
            .cache
            .as_ref()
            .filter(|c| c.list.root.as_deref() == root);
        let fresh = cached.is_some_and(|c| c.at.elapsed() < FRESH_FOR);
        let answer = cached.map(|c| c.list.clone());
        let running = self
            .inflight
            .as_ref()
            .is_some_and(|(r, _)| r.as_deref() == root);
        if !fresh && !running {
            self.spawn(root);
        }
        answer
    }

    fn poll(&mut self) -> Option<SkillList> {
        let (_, rx) = self.inflight.as_ref()?;
        let (list, overrides) = match rx.try_recv() {
            Ok(listing) => listing,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                self.inflight = None;
                return None;
            }
        };
        self.inflight = None;
        self.land(list, overrides)
    }
}

#[cfg(test)]
#[path = "chat_skill_context_tests.rs"]
mod tests;
