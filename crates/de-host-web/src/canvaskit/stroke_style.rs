//! Forwards the active stroke cap / join / dash (#89) to the CanvasKit
//! bridge. The canvas painter scopes the style per node
//! (`de_editor_core::stroke_render_style`); every stroke entry point on
//! [`CanvasKitBackend`] calls [`CanvasKitBackend::sync_stroke_style`]
//! first, and the bridge's `strokePaint` applies it.

use de_editor_core::stroke_render_style::{self, LineCap, LineJoin, StrokeRenderStyle};

use super::backend::CanvasKitBackend;

impl CanvasKitBackend {
    /// Send the active style to the bridge when it differs from the one
    /// the bridge already holds.
    pub(super) fn sync_stroke_style(&mut self) {
        let active = stroke_render_style::active();
        if active.as_deref() == self.sent_stroke_style.as_deref() {
            return;
        }
        match active.as_deref() {
            Some(style) => {
                let (cap, join) = bridge_codes(style);
                let dash = style.dash.as_deref().unwrap_or(&[]);
                self.ck.set_stroke_style(cap, join, dash, style.dash_offset);
            }
            None => self.ck.clear_stroke_style(),
        }
        self.sent_stroke_style = active;
    }
}

/// Indices into the bridge's `STROKE_CAPS` / `STROKE_JOINS`; `-1` keeps
/// the bridge's default round cap / join.
fn bridge_codes(style: &StrokeRenderStyle) -> (i32, i32) {
    let cap = match style.cap {
        None => -1,
        Some(LineCap::Butt) => 0,
        Some(LineCap::Round) => 1,
        Some(LineCap::Square) => 2,
    };
    let join = match style.join {
        None => -1,
        Some(LineJoin::Miter) => 0,
        Some(LineJoin::Round) => 1,
        Some(LineJoin::Bevel) => 2,
    };
    (cap, join)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_codes_match_the_js_tables() {
        let style = StrokeRenderStyle {
            cap: Some(LineCap::Square),
            join: None,
            dash: None,
            dash_offset: 0.0,
        };
        assert_eq!(bridge_codes(&style), (2, -1));
    }
}
