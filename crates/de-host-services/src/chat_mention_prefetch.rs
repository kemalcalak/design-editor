//! Reading staged mentions off the UI thread (#398).
//!
//! A file mention's summary and a skill's instructions need a disk read.
//! [`MentionFileReader`] starts that read on a worker as soon as the file
//! or skill is staged in the draft and stores the result in
//! `ProjectFiles::summaries` / `ChatSkills::instructions`; the send uses it,
//! and waits a frame or two while a read is still running, so the UI thread
//! never touches the disk for a mention.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use de_editor_core::chat_mentions::MentionKind;
use de_editor_core::EditorState;

use crate::chat_mention_context::read_file_mention;
use crate::chat_skill_context::skill_instructions;

/// What a read is for, and what it must still match to land.
enum Target {
    /// Project file `rel` in folder `root`.
    File { root: String, rel: String },
    /// Skill `name`, listed from file `path`.
    Skill { name: String, path: String },
}

struct Read {
    target: Target,
    rx: Receiver<Option<String>>,
}

/// Background reads of staged mentions for the desktop host.
#[derive(Default)]
pub struct MentionFileReader {
    inflight: Vec<Read>,
}

impl MentionFileReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a read is running (the host keeps waking to collect it).
    pub fn is_pending(&self) -> bool {
        !self.inflight.is_empty()
    }

    /// Start reads for newly staged files and skills and land finished
    /// ones. Call once per redraw; with nothing new staged and nothing
    /// running it does nothing (no allocation). True when a read landed.
    pub fn sync(&mut self, state: &mut EditorState) -> bool {
        let landed = self.land(state);
        self.start_files(state);
        self.start_skills(state);
        landed
    }

    fn start_files(&mut self, state: &mut EditorState) {
        let files = &state.chat.project_files;
        let Some(root) = files.root.as_deref() else {
            return;
        };
        let wanted: Vec<String> = state
            .chat
            .pending_mentions
            .iter()
            .filter(|m| m.kind == MentionKind::File && !files.summaries.contains_key(&m.key))
            .map(|m| m.key.clone())
            .collect();
        if wanted.is_empty() {
            return;
        }
        let root = root.to_string();
        for rel in wanted {
            let dir = PathBuf::from(&root);
            let path = rel.clone();
            let target = Target::File {
                root: root.clone(),
                rel: rel.clone(),
            };
            if self.spawn(target, move || Some(read_file_mention(&dir, &path))) {
                state.chat.project_files.summaries.insert(rel, None);
            }
        }
    }

    fn start_skills(&mut self, state: &mut EditorState) {
        let skills = &state.chat.skills;
        let wanted: Vec<_> = state
            .chat
            .pending_mentions
            .iter()
            .filter(|m| m.kind == MentionKind::Skill && !skills.instructions.contains_key(&m.key))
            .map(|m| {
                let option = skills.skills.iter().find(|s| s.name == m.key).cloned();
                (m.key.clone(), option)
            })
            .collect();
        for (name, option) in wanted {
            let Some(option) = option else {
                // Not listed (any more): nothing to read.
                state.chat.skills.instructions.insert(name, Some(None));
                continue;
            };
            let target = Target::Skill {
                name: name.clone(),
                path: option.path.clone(),
            };
            if self.spawn(target, move || skill_instructions(&option)) {
                state.chat.skills.instructions.insert(name, None);
            }
        }
    }

    /// Run `read` on a worker; false when the thread could not start (the
    /// target is then left out, and the send reads it itself).
    fn spawn(
        &mut self,
        target: Target,
        read: impl FnOnce() -> Option<String> + Send + 'static,
    ) -> bool {
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("de-mention-read".into())
            .spawn(move || {
                let _ = tx.send(read());
            });
        match spawned {
            Ok(_) => {
                self.inflight.push(Read { target, rx });
                true
            }
            Err(err) => {
                eprintln!("design-editor: spawn mention read failed: {err}");
                false
            }
        }
    }

    fn land(&mut self, state: &mut EditorState) -> bool {
        if self.inflight.is_empty() {
            return false;
        }
        let mut landed = false;
        self.inflight.retain(|read| {
            let text = match read.rx.try_recv() {
                Ok(text) => Ok(text),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => Err(()),
            };
            landed |= land_one(state, &read.target, text);
            false
        });
        landed
    }
}

/// Store one finished read when what it was for is still current. `Err`:
/// the worker died, so the entry is forgotten and the send reads it.
fn land_one(state: &mut EditorState, target: &Target, text: Result<Option<String>, ()>) -> bool {
    match target {
        Target::File { root, rel } => {
            let files = &mut state.chat.project_files;
            // A read for a folder that is no longer open is dropped.
            if files.root.as_deref() != Some(root.as_str()) {
                return false;
            }
            match text {
                Ok(text) => {
                    files
                        .summaries
                        .insert(rel.clone(), Some(text.unwrap_or_default()));
                }
                Err(()) => {
                    files.summaries.remove(rel);
                }
            }
            true
        }
        Target::Skill { name, path } => {
            let skills = &mut state.chat.skills;
            // Only while the skill is still listed from the same file and
            // still waiting (a new listing cleared the slot).
            let current = skills
                .skills
                .iter()
                .any(|s| &s.name == name && &s.path == path);
            if !current || !matches!(skills.instructions.get(name), Some(None)) {
                return false;
            }
            match text {
                Ok(text) => {
                    skills.instructions.insert(name.clone(), Some(text));
                }
                Err(()) => {
                    skills.instructions.remove(name);
                }
            }
            true
        }
    }
}

#[cfg(test)]
#[path = "chat_mention_prefetch_tests.rs"]
mod tests;
