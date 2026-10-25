//! Shared default geometry for nodes created without explicit bounds.

/// Generic flat-leaf fallback used for non-text TS-compatible inserts.
pub const DEFAULT_LEAF_NODE_SIZE: i32 = 100;

/// Compact default for the Text tool's "Text" placeholder.
pub const DEFAULT_TEXT_NODE_WIDTH: i32 = 48;
pub const DEFAULT_TEXT_NODE_HEIGHT: i32 = 20;

/// Default box of a sticky note / context / prompt annotation (#12).
pub const DEFAULT_NOTE_WIDTH: i32 = 240;
pub const DEFAULT_NOTE_HEIGHT: i32 = 160;

/// Default box of a live browser node (#20): the desktop device preset.
pub const DEFAULT_BROWSER_WIDTH: i32 = 1440;
pub const DEFAULT_BROWSER_HEIGHT: i32 = 900;

/// Default `(width, height)` for a freshly-created flat leaf of `kind`.
pub fn default_leaf_node_size(kind: &str) -> (i32, i32) {
    if let Some(size) = widget_default_size(kind) {
        return size;
    }
    match kind {
        "text" => (DEFAULT_TEXT_NODE_WIDTH, DEFAULT_TEXT_NODE_HEIGHT),
        "note" | "context" | "prompt" => (DEFAULT_NOTE_WIDTH, DEFAULT_NOTE_HEIGHT),
        "browser" => (DEFAULT_BROWSER_WIDTH, DEFAULT_BROWSER_HEIGHT),
        _ => (DEFAULT_LEAF_NODE_SIZE, DEFAULT_LEAF_NODE_SIZE),
    }
}

/// Default `(width, height)` for a freshly-created form widget of the
/// given canonical `snake_case` kind (Phase D2). `None` for any
/// non-widget kind. Sizes mirror the Phase D2 spec table so a tool
/// click and an `InsertNode { kind, .. }` with `width == 0 &&
/// height == 0` both land the same default box.
pub fn widget_default_size(kind: &str) -> Option<(i32, i32)> {
    let size = match kind {
        "text_input" => (240, 40),
        "text_area" => (240, 96),
        "number_input" => (160, 40),
        "select" => (240, 40),
        "radio_group" => (200, 80),
        "switch" => (44, 24),
        "checkbox" => (18, 18),
        "slider" => (240, 20),
        "progress" => (240, 8),
        "tabs" => (320, 200),
        _ => return None,
    };
    Some(size)
}
