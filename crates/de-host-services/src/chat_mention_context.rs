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

use crate::chat_skill_context::MAX_TURN_SKILL_CHARS;
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

/// A file mention's text: the summary the host read when it was staged
/// (`ProjectFiles::summaries`), else read now (hosts without a prefetch).
fn file_mention_text(state: &EditorState, rel: &str, project_root: Option<&Path>) -> String {
    if let Some(Some(text)) = state.chat.project_files.summaries.get(rel) {
        return text.clone();
    }
    match project_root {
        Some(root) => read_file_mention(root, rel),
        None => format!("unreadable — {}", MentionFileError::NoProjectFolder),
    }
}

/// A staged skill's instructions: what the host read when it was staged
/// (`ChatSkills::instructions`), else read now (hosts without a prefetch).
/// `None` when the skill is no longer listed or readable.
fn skill_mention_text(state: &EditorState, name: &str) -> Option<String> {
    let skills = &state.chat.skills;
    if let Some(Some(text)) = skills.instructions.get(name) {
        return text.clone();
    }
    skills
        .skills
        .iter()
        .find(|skill| skill.name == name)
        .and_then(crate::chat_skill_context::skill_instructions)
}

/// The context-block text for project file `rel` (disk I/O).
pub fn read_file_mention(root: &Path, rel: &str) -> String {
    summarize_project_file(root, rel).unwrap_or_else(|err| format!("unreadable — {err}"))
}

/// Resolve every mention of a turn, in order. A skill is looked up in the
/// popup's listing (`state.chat.skills`) and its instructions read from its
/// file; one no longer listed or readable resolves to `None`.
pub fn resolve_mentions(
    state: &EditorState,
    mentions: &[ChatMention],
    project_root: Option<&Path>,
) -> Vec<MentionSummary> {
    mentions
        .iter()
        .map(|mention| {
            let text = match mention.kind {
                MentionKind::File => Some(file_mention_text(state, &mention.key, project_root)),
                MentionKind::Skill => skill_mention_text(state, &mention.key),
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

/// The toast raised when the picked skills' instructions were cut to
/// [`MAX_TURN_SKILL_CHARS`] together.
pub const SKILLS_CUT_TOAST: &str = "chat.skill.cut";

/// Cut the skill summaries of a turn so their instructions stay within
/// [`MAX_TURN_SKILL_CHARS`] together (in pick order). True when any was cut.
pub fn cap_skill_text(summaries: &mut [MentionSummary]) -> bool {
    let mut left = MAX_TURN_SKILL_CHARS;
    let mut cut = false;
    for summary in summaries
        .iter_mut()
        .filter(|s| s.mention.kind == MentionKind::Skill)
    {
        let Some(text) = summary.text.as_mut() else {
            continue;
        };
        let chars = text.chars().count();
        if chars <= left {
            left -= chars;
            continue;
        }
        cut = true;
        if left == 0 {
            summary.text = Some("[left out: the turn's skill limit was reached]".to_string());
            continue;
        }
        let kept: String = text.chars().take(left).collect();
        *text = format!("{kept}\n[…cut at the turn's skill limit]");
        left = 0;
    }
    cut
}

/// Take the active tab's staged mentions for the turn being sent and
/// return their context block. Each turn reads its files and skills afresh,
/// so the prefetched texts it used are dropped. The picked skills share one
/// budget; when it cuts them, a toast says so.
pub fn take_turn_mentions(state: &mut EditorState, now_ms: u64) -> String {
    let mentions = std::mem::take(&mut state.chat.pending_mentions);
    if mentions.is_empty() {
        return String::new();
    }
    let root = state
        .chat
        .project_files
        .root
        .as_deref()
        .map(std::path::PathBuf::from);
    let mut summaries = resolve_mentions(state, &mentions, root.as_deref());
    if cap_skill_text(&mut summaries) {
        state.editor_ui.show_toast(
            SKILLS_CUT_TOAST,
            vec![("count".to_string(), MAX_TURN_SKILL_CHARS.to_string())],
            de_editor_core::editor_toast::EditorToastLevel::Info,
            now_ms,
        );
    }
    for mention in &mentions {
        match mention.kind {
            MentionKind::File => {
                state.chat.project_files.summaries.remove(&mention.key);
            }
            MentionKind::Skill => {
                state.chat.skills.instructions.remove(&mention.key);
            }
            _ => {}
        }
    }
    context_block(&summaries)
}

#[cfg(test)]
#[path = "chat_mention_context_tests.rs"]
mod tests;
