//! The provider-bound text of a chat turn (#398): what the user typed plus
//! the context the composer staged for it. Split off `chat_session_launch.rs`
//! at the 800-line cap.
//!
//! The launch path routes on the typed text alone (`routing_text`), so a
//! mentioned layer called "New page" or an attached file's contents cannot
//! steer intent classification; every provider receives the composed text.

use de_host_native::WidgetHostNative;
use de_host_services::chat_attachment_prepare::{model_sees_images, prepare_turn_attachments};
use de_host_services::chat_mention_context::mention_context_for_turn;

/// Drain the turn's staged context and return `routing_text` with it
/// appended:
///
/// - `@`-mentions resolve to compact summaries (document items in core,
///   files from the document's folder);
/// - attachments split: images stay `pending_attachments` for a model that
///   sees them (every route takes them from there), while text files, SVG
///   and images a text-only model cannot see become captions.
pub(super) fn compose_turn_text(host: &mut WidgetHostNative, routing_text: &str) -> String {
    let mentions = std::mem::take(&mut host.editor_state_mut().chat.pending_mentions);
    let project_root = host
        .editor_state()
        .chat
        .project_files
        .root
        .clone()
        .map(std::path::PathBuf::from);
    let sees_images = host
        .editor_state()
        .chat
        .selected_model_entry()
        .is_none_or(model_sees_images);
    let staged = std::mem::take(&mut host.editor_state_mut().chat.pending_attachments);
    let prepared = prepare_turn_attachments(staged, sees_images);
    host.editor_state_mut().chat.pending_attachments = prepared.send;
    let mention_block =
        mention_context_for_turn(host.editor_state(), &mentions, project_root.as_deref());
    format!("{routing_text}{mention_block}{}", prepared.captions)
}
