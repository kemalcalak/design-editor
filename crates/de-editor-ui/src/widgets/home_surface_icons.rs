//! The lucide glyphs Home needs that the shared `icons` catalogue does
//! not carry. Kept here rather than in `icons.rs`, which sits at the
//! 800-line cap; the d-strings are lucide's 24×24 stroke art.

use crate::widgets::icons::{draw_icon, Icon};
use crate::{Color, Point2D, RenderBackend};

/// A glyph Home paints: a shared catalogue icon or one of the extras
/// below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HomeGlyph {
    Shared(Icon),
    Monitor,
    Tablet,
    File,
    List,
    Plug,
    SwatchBook,
    LayoutTemplate,
    AppWindow,
    Presentation,
    PanelsTopLeft,
}

impl HomeGlyph {
    fn paths(self) -> &'static [&'static str] {
        match self {
            Self::Shared(_) => &[],
            Self::Monitor => &[
                "M4 3h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
                "M8 21h8",
                "M12 17v4",
            ],
            Self::Tablet => &[
                "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z",
                "M12 18h.01",
            ],
            Self::File => &[
                "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z",
                "M14 2v4a2 2 0 0 0 2 2h4",
            ],
            Self::List => &[
                "M3 12h.01",
                "M3 18h.01",
                "M3 6h.01",
                "M8 12h13",
                "M8 18h13",
                "M8 6h13",
            ],
            Self::Plug => &[
                "M12 22v-5",
                "M9 8V2",
                "M15 8V2",
                "M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z",
            ],
            Self::SwatchBook => &[
                "M11 17a4 4 0 0 1-8 0V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2Z",
                "M16.7 13H19a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H7",
                "M7 17h.01",
                "m11 8 2.3-2.3a2.4 2.4 0 0 1 3.404.004L18.6 7.6a2.4 2.4 0 0 1 .026 3.434L9.9 19.8",
            ],
            Self::LayoutTemplate => &[
                "M4 3h16a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z",
                "M4 14h7a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-5a1 1 0 0 1 1-1z",
                "M17 14h3a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-5a1 1 0 0 1 1-1z",
            ],
            Self::AppWindow => &[
                "M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
                "M10 4v4",
                "M2 8h20",
                "M6 4v4",
            ],
            Self::Presentation => &[
                "M2 3h20",
                "M21 3v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V3",
                "m7 21 5-5 5 5",
            ],
            Self::PanelsTopLeft => &[
                "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z",
                "M3 9h18",
                "M9 21V9",
            ],
        }
    }
}

/// Stroke `glyph` into the `size`-px square at `top_left`.
pub(crate) fn draw_glyph(
    backend: &mut dyn RenderBackend,
    glyph: HomeGlyph,
    top_left: Point2D,
    size: f32,
    color: Color,
    stroke_width: f32,
) {
    if let HomeGlyph::Shared(icon) = glyph {
        draw_icon(backend, icon, top_left, size, color, stroke_width);
        return;
    }
    for d in glyph.paths() {
        backend.stroke_svg_path(d, top_left, size, color, stroke_width);
    }
}
