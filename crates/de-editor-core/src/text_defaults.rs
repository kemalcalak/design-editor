//! Defaults for a text node that leaves a style unset. The canvas painter,
//! the inline editor, the exports and the layout measure all read these, so
//! a `fit_content` box is sized for the text the canvas paints in it.

/// Font size when none is authored. jian's layout measure and renderer use
/// the same value (#197; the painter used 13 before).
pub const FONT_SIZE: f32 = 14.0;

/// Weight when none is authored.
pub const FONT_WEIGHT: u16 = 400;

/// Line advance, as a multiple of the font size, when no `lineHeight` is
/// authored.
pub const LINE_HEIGHT: f32 = 1.2;

/// Family when none is authored.
pub const FONT_FAMILY: &str = "system-ui";
