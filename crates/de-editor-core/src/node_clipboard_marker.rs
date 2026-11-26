//! The system-clipboard marker written when nodes are copied or cut (#436).
//!
//! Node copy fills the in-memory node clipboard (`EditorState::clipboard`),
//! but canvas paste reads the system clipboard first (HTML > image > SVG
//! text > editor nodes). Without touching the system clipboard, whatever
//! was copied before — a Copy as SVG, an image, a browser selection — kept
//! winning over the nodes just copied. So a user copy / cut also puts this
//! marker on the system clipboard, as HTML only:
//!
//! - writing it replaces every older flavour, so nothing stale outranks it;
//! - paste finds it in the HTML flavour and goes straight to the node
//!   clipboard (Figma behaviour: the last copy is what pastes);
//! - it carries no plain-text flavour, so a text field, a text editor or a
//!   terminal pastes nothing rather than a marker string;
//! - anything copied in another app afterwards replaces it and pastes as
//!   before.
//!
//! The payload is an empty element whose `data-design-editor` attribute
//! names the content and its version. The node clipboard is per editor
//! process, so today the marker only says "the editor's node clipboard is
//! current"; the attribute is where serialized nodes would go for pasting
//! across processes. MCP `copy_selected` does not write it — an agent must
//! not overwrite the user's system clipboard.

/// The HTML the current editor writes.
pub const NODE_CLIPBOARD_MARKER_HTML: &str = r#"<span data-design-editor="nodes/v1"></span>"#;

/// The part every version of the marker carries. Matched as a substring:
/// platforms wrap clipboard HTML (macOS and Windows add a document or
/// fragment header around it).
const MARKER_ATTRIBUTE: &str = r#"data-design-editor="nodes/"#;

/// Whether system-clipboard `html` holds the editor's node-copy marker
/// (any version).
pub fn is_node_clipboard_marker(html: &str) -> bool {
    html.contains(MARKER_ATTRIBUTE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_marker_is_recognised() {
        assert!(is_node_clipboard_marker(NODE_CLIPBOARD_MARKER_HTML));
    }

    #[test]
    fn the_marker_is_recognised_inside_platform_wrappers() {
        // arboard on macOS.
        let mac = format!(
            r#"<html><head><meta http-equiv="content-type" content="text/html; charset=utf-8"></head><body>{NODE_CLIPBOARD_MARKER_HTML}</body></html>"#
        );
        assert!(is_node_clipboard_marker(&mac));
        // Windows CF_HTML.
        let windows = format!(
            "Version:0.9\r\nStartHTML:0000000105\r\nEndHTML:0000000199\r\n<html><body>\r\n<!--StartFragment-->{NODE_CLIPBOARD_MARKER_HTML}<!--EndFragment-->\r\n</body></html>"
        );
        assert!(is_node_clipboard_marker(&windows));
    }

    #[test]
    fn other_marker_versions_are_recognised() {
        assert!(is_node_clipboard_marker(
            r#"<span data-design-editor="nodes/v2" data-nodes="e30="></span>"#
        ));
    }

    #[test]
    fn ordinary_html_is_not_the_marker() {
        assert!(!is_node_clipboard_marker(""));
        assert!(!is_node_clipboard_marker("<p>just words</p>"));
        assert!(!is_node_clipboard_marker(
            r#"<span data-design-editor="theme/v1"></span>"#
        ));
        assert!(!is_node_clipboard_marker("design-editor:nodes/v1"));
    }
}
