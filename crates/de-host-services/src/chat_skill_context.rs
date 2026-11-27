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
//! picked up without a restart. The same listing installs the skills named
//! like a built-in skill as its replacements
//! (`de_ai_skills::loader::set_skill_overrides`). A picked skill's
//! instructions are read from its file at send.

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
/// Characters of a skill's instructions handed to the model in one turn.
pub const MAX_SKILL_CONTEXT_CHARS: usize = 12_000;
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
            shadows_user: skill.shadows_user,
            overrides_builtin: skill.overrides_builtin,
        })
        .collect()
}

/// The skills of `set` that replace a built-in skill, with their
/// instructions read. One that cannot be read is logged and left out, so
/// the built-in stays.
pub fn builtin_overrides(set: &UserSkillSet) -> Vec<SkillEntry> {
    set.skills
        .iter()
        .filter(|skill| skill.overrides_builtin)
        .filter_map(|skill| match read_skill_body(&skill.path) {
            Ok(content) => Some(SkillEntry {
                meta: skill.meta.clone(),
                content,
            }),
            Err(err) => {
                eprintln!(
                    "design-editor: skipped skill {}: {err}",
                    skill.path.display()
                );
                None
            }
        })
        .collect()
}

/// List the skills for `project_root` and install the built-in overrides
/// they carry. Disk I/O: runs on the scanner's worker thread.
pub fn list_chat_skills(project_root: Option<&Path>) -> SkillList {
    let set = load_chat_skills(&skill_folders(project_root));
    set_skill_overrides(builtin_overrides(&set));
    SkillList {
        root: project_root.map(Path::to_path_buf),
        skills: Arc::from(skill_options(&set)),
    }
}

/// A picked skill's text for the context block: its description and its
/// instructions read from its file, cut at [`MAX_SKILL_CONTEXT_CHARS`].
/// `None` when the file is gone or no longer a valid skill.
pub fn skill_instructions(skill: &SkillOption) -> Option<String> {
    let body = match read_skill_body(Path::new(&skill.path)) {
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

struct Cached {
    list: SkillList,
    at: Instant,
}

/// Background, cached [`SkillListSource`] for the desktop host.
#[derive(Default)]
pub struct ChatSkillScanner {
    cache: Option<Cached>,
    inflight: Option<(Option<PathBuf>, Receiver<SkillList>)>,
}

impl ChatSkillScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a listing is running on its worker (the host keeps waking
    /// to collect it).
    pub fn is_pending(&self) -> bool {
        self.inflight.is_some()
    }

    fn spawn(&mut self, root: Option<&Path>) {
        let (tx, rx) = mpsc::channel();
        let dir = root.map(Path::to_path_buf);
        let spawned = thread::Builder::new()
            .name("de-skill-scan".into())
            .spawn(move || {
                // The receiver may be gone (another folder was opened); the
                // listing is then simply dropped.
                let _ = tx.send(list_chat_skills(dir.as_deref()));
            });
        match spawned {
            Ok(_) => self.inflight = Some((root.map(Path::to_path_buf), rx)),
            Err(err) => eprintln!("design-editor: spawn skill scan failed: {err}"),
        }
    }
}

impl SkillListSource for ChatSkillScanner {
    fn request(&mut self, root: Option<&Path>) -> Option<SkillList> {
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
        let list = match rx.try_recv() {
            Ok(list) => list,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                self.inflight = None;
                return None;
            }
        };
        self.inflight = None;
        self.cache = Some(Cached {
            list: list.clone(),
            at: Instant::now(),
        });
        Some(list)
    }
}

#[cfg(test)]
#[path = "chat_skill_context_tests.rs"]
mod tests;
