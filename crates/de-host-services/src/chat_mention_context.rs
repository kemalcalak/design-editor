//! Host half of chat `@`-mentions (#398): resolving a turn's mentions into
//! the context block that rides the provider-bound user message. The folder
//! listing behind the popup is `crate::chat_mention_scan`.
//!
//! Document mentions resolve in `de_editor_core::chat_mentions::summary`;
//! files are read here, because `de-editor-core` stays I/O-free. Reads are
//! bounded so a mention can never stall the turn on a huge file.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use de_editor_core::chat_mentions::{
    context_block, summarize_document_mention, ChatMention, MentionKind, MentionSummary,
};
use de_editor_core::EditorState;

use crate::chat_text_excerpt::text_excerpt;

pub use crate::chat_mention_context_error::MentionFileError;

/// Bytes read from a mentioned file.
const MAX_READ_BYTES: u64 = 64 * 1024;

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
