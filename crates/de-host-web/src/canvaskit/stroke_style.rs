//! Forwards the active stroke cap / join / dash (#89) to the CanvasKit
//! bridge. The canvas painter scopes the style per node
//! (`de_editor_core::stroke_render_style`); every stroke entry point on
//! [`CanvasKitBackend`] calls [`CanvasKitBackend::sync_stroke_style`]
//! first, and the bridge's `strokePaint` applies it.
//!
//! The bridge strokes everything round by default, while the native
//! backend draws rects and ovals with skia's butt cap + miter join. While
//! a style is active, an unset cap / join is therefore sent as the
//! native default of the primitive being stroked, so a dashed rect has
//! the same dash ends on desktop and web. Unstyled chrome is untouched.

use de_editor_core::stroke_render_style::{self, LineCap, LineJoin, StrokeRenderStyle};

use super::backend::CanvasKitBackend;

/// The native default cap / join of the primitive about to be stroked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StrokeDefaults {
    /// Lines, polygons and SVG paths: round cap + round join.
    Round,
    /// Rects, round rects and ovals: butt cap + miter join.
    Sharp,
}

impl CanvasKitBackend {
    /// Send the active style to the bridge when it differs from the one
    /// the bridge already holds.
    pub(super) fn sync_stroke_style(&mut self, defaults: StrokeDefaults) {
        let active = stroke_render_style::active();
        let unchanged = match (&active, &self.sent_stroke_style) {
            (None, None) => true,
            (Some(next), Some((sent, sent_defaults))) => {
                **next == **sent && defaults == *sent_defaults
            }
            _ => false,
        };
        if unchanged {
            return;
        }
        match active.as_deref() {
            Some(style) => {
                let (cap, join) = bridge_codes(style, defaults);
                let dash = style.dash.as_deref().unwrap_or(&[]);
                self.ck.set_stroke_style(cap, join, dash, style.dash_offset);
            }
            None => self.ck.clear_stroke_style(),
        }
        self.sent_stroke_style = active.map(|style| (style, defaults));
    }
}

/// Indices into the bridge's `STROKE_CAPS` / `STROKE_JOINS`.
fn bridge_codes(style: &StrokeRenderStyle, defaults: StrokeDefaults) -> (i32, i32) {
    let (default_cap, default_join) = match defaults {
        StrokeDefaults::Round => (LineCap::Round, LineJoin::Round),
        StrokeDefaults::Sharp => (LineCap::Butt, LineJoin::Miter),
    };
    let cap = match style.cap.unwrap_or(default_cap) {
        LineCap::Butt => 0,
        LineCap::Round => 1,
        LineCap::Square => 2,
    };
    let join = match style.join.unwrap_or(default_join) {
        LineJoin::Miter => 0,
        LineJoin::Round => 1,
        LineJoin::Bevel => 2,
    };
    (cap, join)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_fields_take_the_native_primitive_default() {
        let style = StrokeRenderStyle {
            cap: Some(LineCap::Square),
            join: None,
            dash: Some(vec![4.0, 4.0]),
            dash_offset: 0.0,
        };
        assert_eq!(bridge_codes(&style, StrokeDefaults::Sharp), (2, 0));
        assert_eq!(bridge_codes(&style, StrokeDefaults::Round), (2, 1));
    }

    /// The codes index these exact JS tables; keep them in step.
    #[test]
    fn bridge_tables_match_the_codes() {
        let js = include_str!("../op_ck_bridge.js");
        assert!(js.contains(
            "const STROKE_CAPS = [CK.StrokeCap.Butt, CK.StrokeCap.Round, CK.StrokeCap.Square];"
        ));
        assert!(js.contains(
            "const STROKE_JOINS = [CK.StrokeJoin.Miter, CK.StrokeJoin.Round, CK.StrokeJoin.Bevel];"
        ));
    }
}
