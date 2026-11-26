//! The system-clipboard marker written when nodes are copied or cut (#436).
//!
//! Node copy fills the in-memory node clipboard (`EditorState::clipboard`),
//! but canvas paste reads the system clipboard first (HTML > image > SVG
//! text > editor nodes). Without touching the system clipboard, whatever
//! was copied before — a Copy as SVG, an image, a browser selection — kept
//! winning over the nodes just copied. So a user copy / cut also puts this
//! marker on the system clipboard as plain text:
//!
//! - writing it replaces every older flavour, so nothing stale outranks it;
//! - paste recognises it and goes straight to the node clipboard (Figma
//!   behaviour: the last copy is what pastes);
//! - text inputs treat it as empty, so a copied layer never pastes this
//!   string into a field;
//! - anything copied in another app afterwards replaces the marker and
//!   pastes as before.
//!
//! The marker is a plain, versioned token rather than serialized nodes: the
//! node clipboard is per editor process, so the text only has to say "the
//! editor's node clipboard is current", and it is cheap to write on every
//! Cmd+C. MCP `copy_selected` does not write it — an agent must not
//! overwrite the user's system clipboard.

/// Prefix every version of the marker starts with.
const MARKER_PREFIX: &str = "design-editor:nodes";

/// The marker text the current editor writes.
pub const NODE_CLIPBOARD_MARKER: &str = "design-editor:nodes/v1";

/// Whether system-clipboard `text` is the editor's node-copy marker (any
/// version, surrounding whitespace ignored — some platforms append a
/// newline on round-trip).
pub fn is_node_clipboard_marker(text: &str) -> bool {
    let text = text.trim();
    text.strip_prefix(MARKER_PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_marker_is_recognised() {
        assert!(is_node_clipboard_marker(NODE_CLIPBOARD_MARKER));
        assert!(is_node_clipboard_marker(&format!(
            "\n{NODE_CLIPBOARD_MARKER}\r\n"
        )));
    }

    #[test]
    fn other_marker_versions_are_recognised() {
        assert!(is_node_clipboard_marker("design-editor:nodes"));
        assert!(is_node_clipboard_marker("design-editor:nodes/v2"));
    }

    #[test]
    fn ordinary_text_is_not_the_marker() {
        assert!(!is_node_clipboard_marker(""));
        assert!(!is_node_clipboard_marker("just words"));
        assert!(!is_node_clipboard_marker("design-editor:nodesX"));
        assert!(!is_node_clipboard_marker(
            "see design-editor:nodes/v1 for details"
        ));
        assert!(!is_node_clipboard_marker("<svg/>"));
    }
}
