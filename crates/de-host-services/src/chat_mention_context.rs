//! Host half of chat `@`-mentions (#398): listing the project folder for
//! the popup, and resolving a turn's mentions into the context block that
//! rides the provider-bound user message.
//!
//! Document mentions resolve in `de_editor_core::chat_mentions::summary`;
//! files are read here, because `de-editor-core` stays I/O-free. Both the
//! scan and the reads are bounded so a mention can never stall the UI
//! thread on a huge folder or file.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use de_editor_core::chat_mentions::{
    context_block, summarize_document_mention, ChatMention, MentionKind, MentionSummary,
};
use de_editor_core::EditorState;

use crate::chat_text_excerpt::text_excerpt;

pub use crate::chat_mention_context_error::MentionFileError;

/// Files the popup is offered at most.
pub const MAX_PROJECT_FILES: usize = 2_000;
/// Folder levels below the project root the scan descends.
pub const MAX_SCAN_DEPTH: usize = 4;
/// Bytes read from a mentioned file.
const MAX_READ_BYTES: u64 = 64 * 1024;

/// Folders never worth offering: VCS internals, dependency and build
/// output trees.
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "dist", "build", "__pycache__"];

fn skipped(name: &str, is_dir: bool) -> bool {
    name.starts_with('.') || (is_dir && SKIPPED_DIRS.contains(&name))
}

/// Project-relative paths (`/`-separated, sorted) of the files under
/// `root`, breadth-first, at most [`MAX_PROJECT_FILES`] and
/// [`MAX_SCAN_DEPTH`] levels deep. Hidden entries and dependency / build
/// folders are skipped; unreadable folders are passed over silently.
pub fn scan_project_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    let mut next = 0;
    while next < queue.len() && files.len() < MAX_PROJECT_FILES {
        let (dir, depth) = queue[next].clone();
        next += 1;
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if skipped(name, kind.is_dir()) || kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if depth < MAX_SCAN_DEPTH {
                    queue.push((path, depth + 1));
                }
            } else if kind.is_file() {
                if let Some(rel) = relative_slash_path(root, &path) {
                    files.push(rel);
                    if files.len() >= MAX_PROJECT_FILES {
                        break;
                    }
                }
            }
        }
    }
    files.sort();
    files
}

fn relative_slash_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    Some(parts?.join("/"))
}

/// Resolve `rel` under `root`, refusing anything that escapes it (`..`,
/// an absolute path, a symlink pointing outside).
fn resolve_in_root(root: &Path, rel: &str) -> Result<PathBuf, MentionFileError> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(MentionFileError::OutsideProject {
            path: rel.to_string(),
        });
    }
    let root = root.canonicalize().map_err(|source| MentionFileError::Io {
        path: rel.to_string(),
        source,
    })?;
    let full = root
        .join(rel_path)
        .canonicalize()
        .map_err(|source| MentionFileError::Io {
            path: rel.to_string(),
            source,
        })?;
    if !full.starts_with(&root) {
        return Err(MentionFileError::OutsideProject {
            path: rel.to_string(),
        });
    }
    Ok(full)
}

/// Summarize project file `rel`: its size, and for UTF-8 text the first
/// lines in a fenced block (cut by `chat_text_excerpt::text_excerpt`). Binary files are described, not
/// quoted — images should be attached instead.
pub fn summarize_project_file(root: &Path, rel: &str) -> Result<String, MentionFileError> {
    let full = resolve_in_root(root, rel)?;
    let io = |source| MentionFileError::Io {
        path: rel.to_string(),
        source,
    };
    let size = fs::metadata(&full).map_err(io)?.len();
    let mut bytes = Vec::new();
    fs::File::open(&full)
        .map_err(io)?
        .take(MAX_READ_BYTES)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let media = crate::chat_attachment::media_type_for_path(&full);
    let text = if bytes.contains(&0) {
        None
    } else {
        match std::str::from_utf8(&bytes) {
            Ok(text) => Some(text),
            // A read cut mid-character at the byte cap is still text.
            Err(err) if size > MAX_READ_BYTES && err.error_len().is_none() => {
                std::str::from_utf8(&bytes[..err.valid_up_to()]).ok()
            }
            Err(_) => None,
        }
    };
    let Some(text) = text else {
        let hint = if media.starts_with("image/") {
            " — attach the image to send its pixels"
        } else {
            ""
        };
        return Ok(format!("binary file ({media}), {size} bytes{hint}"));
    };
    let (body, truncated) = text_excerpt(text);
    let more = if truncated || size > MAX_READ_BYTES {
        ", excerpt"
    } else {
        ""
    };
    Ok(format!("text file, {size} bytes{more}:\n```\n{body}\n```"))
}

/// Resolve every mention of a turn, in order.
pub fn resolve_mentions(
    state: &EditorState,
    mentions: &[ChatMention],
    project_root: Option<&Path>,
) -> Vec<MentionSummary> {
    mentions
        .iter()
        .map(|mention| {
            let text = match mention.kind {
                MentionKind::File => Some(match project_root {
                    Some(root) => summarize_project_file(root, &mention.key)
                        .unwrap_or_else(|err| format!("unreadable — {err}")),
                    None => format!("unreadable — {}", MentionFileError::NoProjectFolder),
                }),
                _ => summarize_document_mention(state, mention),
            };
            MentionSummary {
                mention: mention.clone(),
                text,
            }
        })
        .collect()
}

/// The context block for a turn's mentions (empty when there are none),
/// ready to append to the provider-bound user message.
pub fn mention_context_for_turn(
    state: &EditorState,
    mentions: &[ChatMention],
    project_root: Option<&Path>,
) -> String {
    if mentions.is_empty() {
        return String::new();
    }
    context_block(&resolve_mentions(state, mentions, project_root))
}

#[cfg(test)]
#[path = "chat_mention_context_tests.rs"]
mod tests;
