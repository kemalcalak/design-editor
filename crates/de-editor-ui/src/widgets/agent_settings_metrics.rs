//! The settings modal's spacing scale — one named ladder every
//! `agent_settings_*` surface measures against.
//!
//! Before this module the modal carried four different section gaps, three
//! copies of the empty-state height, two ideas of how far in a row's
//! avatar sits, and a 27 pt headline on top of each tab. Nothing was
//! wrong in isolation; together they read as a big, loose dialog rather
//! than a dense one. The fix is not "make each number smaller" — it is
//! having ONE set of numbers, so a row on the MCP tab and a row on the
//! Agents tab are the same row.
//!
//! Anything vertical or horizontal a settings surface needs comes from
//! here. A literal in a sibling module is a bug unless it is the size of
//! a specific control (a 36 px switch, a 96 px button), which is that
//! control's business, not the layout's.

/// Clear space a row keeps between its text and whatever control sits on
/// the right, so a long label ellipsizes instead of colliding.
pub(super) const ROW_TEXT_GAP: f32 = 16.0;

/// Box height of a list row, by how many lines of text it carries. Two
/// numbers, not one: a single `ROW_HEIGHT` for both shapes is exactly
/// what let a two-line row's second line escape its box.
///
/// These are the modal's densest legible rows — the two-line box holds a
/// 14 pt label over a 12 pt description with a clear gap between the
/// label's descender and the description's ascender, and the one-line box
/// centres its single label. Shrinking either without shrinking the fonts
/// in `agent_settings_rows` puts ink outside the box; the traversal audit
/// in `agent_settings_row_layout_tests` catches that.
pub(super) const ROW_H_ONE_LINE: f32 = 44.0;
pub(super) const ROW_H_TWO_LINE: f32 = 54.0;

/// Gap between two sections of a tab body.
pub(super) const SECTION_GAP: f32 = 24.0;
/// Band a section header (title, optional trailing action) occupies
/// before its first row.
pub(super) const SECTION_HEADER_H: f32 = 28.0;
/// Clear space every tab reserves under its last row when reporting its
/// scrollable height. Without it the final row sits flush against the
/// modal's bottom edge at full scroll and reads as clipped.
pub(super) const CONTENT_TAIL_PAD: f32 = 24.0;

/// Horizontal inset from the modal edge to the content column.
pub(super) const CONTENT_PAD_X: f32 = 28.0;
/// Clear space between the content column's bottom and the modal edge.
pub(super) const CONTENT_PAD_BOTTOM: f32 = 20.0;
