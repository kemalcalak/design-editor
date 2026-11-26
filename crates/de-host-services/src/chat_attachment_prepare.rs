//! Per-turn attachment preparation (#398): decide, for each staged
//! attachment, whether it travels as an image block or as a caption in the
//! user message.
//!
//! - Raster images go to the provider as images when the selected model can
//!   see them (`model_sees_images`); otherwise the model gets a caption that
//!   says an image was attached and that it cannot see it, instead of a
//!   request the provider rejects or silently drops.
//! - SVG is XML: it is quoted as text, never sent as an image block (no
//!   provider accepts `image/svg+xml` image input).
//! - Text files (UTF-8, no NUL) are quoted as a fenced excerpt with a caption
//!   line naming the file, so every transport, CLI or HTTP, sees the same
//!   words.
//! - Anything else is described (name, type, size) and not sent.

use de_editor_core::chat::ChatAttachment;

use crate::chat_text_excerpt::text_excerpt;

/// What a turn sends: the attachments that stay attachments (images for a
/// vision model) and the caption block appended to the user message.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreparedAttachments {
    pub send: Vec<ChatAttachment>,
    /// Empty when nothing became a caption.
    pub captions: String,
}

fn is_svg(att: &ChatAttachment) -> bool {
    att.media_type == "image/svg+xml" || att.name.to_ascii_lowercase().ends_with(".svg")
}

fn as_text(att: &ChatAttachment) -> Option<&str> {
    if att.data.contains(&0) {
        return None;
    }
    std::str::from_utf8(&att.data).ok()
}

fn size_label(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Split a turn's staged attachments into what is sent as-is and what is
/// folded into the message as captions.
pub fn prepare_turn_attachments(
    attachments: Vec<ChatAttachment>,
    model_sees_images: bool,
) -> PreparedAttachments {
    let mut send = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for att in attachments {
        let size = size_label(att.data.len());
        if att.is_image() && !is_svg(&att) {
            if model_sees_images {
                send.push(att);
            } else {
                lines.push(format!(
                    "- image \"{}\" ({}, {size}): the selected model cannot see images, \
                     so its pixels were not sent. Ask the user to describe it if it matters.",
                    att.name, att.media_type
                ));
            }
            continue;
        }
        match as_text(&att) {
            Some(text) => {
                let (body, truncated) = text_excerpt(text);
                let cut = if truncated { ", excerpt" } else { "" };
                lines.push(format!(
                    "- file \"{}\" ({size}{cut}):\n```\n{body}\n```",
                    att.name
                ));
            }
            None => lines.push(format!(
                "- file \"{}\" ({}, {size}): binary content, not readable as text, not sent.",
                att.name, att.media_type
            )),
        }
    }
    let captions = if lines.is_empty() {
        String::new()
    } else {
        format!("\n\n---\nAttached files:\n{}", lines.join("\n"))
    };
    PreparedAttachments { send, captions }
}

/// Whether the selected chat model takes image input.
///
/// Agent CLIs and ACP agents read an attached image through their own tools
/// (a temp file path), so they count as seeing it. A built-in (API key)
/// model sends it as an inline image block, which only vision models accept;
/// that decision is `de_chat_agent`'s single model table.
pub fn model_sees_images(entry: &de_editor_core::chat::ModelEntry) -> bool {
    if entry.builtin_provider_id.is_none() {
        return true;
    }
    de_chat_agent::chat_agent_context::model_supports_inline_images(&entry.value)
}

#[cfg(test)]
#[path = "chat_attachment_prepare_tests.rs"]
mod tests;
