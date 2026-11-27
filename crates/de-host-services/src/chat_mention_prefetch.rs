//! Reading mentioned project files off the UI thread (#398).
//!
//! A file mention's summary needs a disk read. [`MentionFileReader`] starts
//! that read on a worker as soon as the file is staged in the draft and
//! stores the result in `ProjectFiles::summaries`; the send uses it, and
//! waits a frame or two while a read is still running, so the UI thread
//! never touches the disk for a mention.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use de_editor_core::chat_mentions::MentionKind;
use de_editor_core::EditorState;

use crate::chat_mention_context::read_file_mention;

struct Read {
    root: String,
    rel: String,
    rx: Receiver<String>,
}

/// Background reads of staged file mentions for the desktop host.
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

    /// Start reads for newly staged file mentions and land finished ones.
    /// Call once per redraw; with no file mention staged and nothing
    /// running it does nothing (no allocation). True when a read landed.
    pub fn sync(&mut self, state: &mut EditorState) -> bool {
        let landed = self.land(state);
        let files = &state.chat.project_files;
        let Some(root) = files.root.as_deref() else {
            return landed;
        };
        let wanted: Vec<String> = state
            .chat
            .pending_mentions
            .iter()
            .filter(|m| m.kind == MentionKind::File && !files.summaries.contains_key(&m.key))
            .map(|m| m.key.clone())
            .collect();
        if wanted.is_empty() {
            return landed;
        }
        let root = root.to_string();
        for rel in wanted {
            let (tx, rx) = mpsc::channel();
            let dir = PathBuf::from(&root);
            let path = rel.clone();
            let spawned = thread::Builder::new()
                .name("de-mention-read".into())
                .spawn(move || {
                    let _ = tx.send(read_file_mention(&dir, &path));
                });
            match spawned {
                Ok(_) => {
                    state.chat.project_files.summaries.insert(rel.clone(), None);
                    self.inflight.push(Read {
                        root: root.clone(),
                        rel,
                        rx,
                    });
                }
                // Left out of `summaries`: the send then reads it itself.
                Err(err) => eprintln!("design-editor: spawn mention read failed: {err}"),
            }
        }
        landed
    }

    fn land(&mut self, state: &mut EditorState) -> bool {
        if self.inflight.is_empty() {
            return false;
        }
        let mut landed = false;
        let files = &mut state.chat.project_files;
        self.inflight.retain(|read| {
            let text = match read.rx.try_recv() {
                Ok(text) => Some(text),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => None,
            };
            // A read for a folder that is no longer open is dropped.
            if files.root.as_deref() == Some(read.root.as_str()) {
                match text {
                    Some(text) => {
                        files.summaries.insert(read.rel.clone(), Some(text));
                    }
                    // The worker died: forget it so the send reads itself.
                    None => {
                        files.summaries.remove(&read.rel);
                    }
                }
                landed = true;
            }
            false
        });
        landed
    }
}

#[cfg(test)]
#[path = "chat_mention_prefetch_tests.rs"]
mod tests;
