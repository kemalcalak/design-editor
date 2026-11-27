//! The write an MCP call hands its host to apply (split out of
//! `mcp_serve.rs` at the 800-line cap).

use de_editor_core::EditorCommand;

/// One MCP write as the host should apply it.
pub struct McpWrite<'a> {
    /// The command to apply: the tool's own, or its root-placed form (#37).
    pub command: &'a EditorCommand,
    /// The tool's own command when `command` is the placed form. Placement
    /// names the new roots by the ids the insert will mint on the state it
    /// was computed against, so a host whose document may have moved on
    /// since (the live UI thread) applies this instead when the revision
    /// differs: an unplaced insert, never a move of somebody else's node.
    pub unplaced: Option<&'a EditorCommand>,
    /// `document_revision()` of the state placement was computed against.
    pub revision: u64,
}
