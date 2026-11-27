//! The gallery panels' shared control chrome: a segmented control, a chip,
//! and a filled button.
//!
//! Three painters, one vocabulary. Before this the Asset Center drew its own
//! tab pills, its own filter pills, its own card buttons and its own
//! generate button, each with its own idea of what "selected" and "hovered"
//! look like — so the panel had four selection idioms and a user had to
//! learn which one they were looking at.
//!
//! Geometry and paint are split the way the rest of the panel splits them:
//! [`segment_rects`] is the single source both paint and hit-testing read,
//! because a segment drawn from one set of numbers and pressed from another
//! is how a control ends up half a pixel from where it looks.

use crate::theme::Theme;
use crate::widgets::label_widths::LabelWidths;
use crate::widgets::panel_control_metrics::{
    control_fill, mix, CHIP_LABEL_SIZE, CHIP_PAD_X, CHIP_RADIUS, SEGMENT_MIN_W, SEGMENT_TRACK_PAD,
};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// Space either side of a chip's or button's label inside its box.
const LABEL_INSET: f32 = 5.0;

/// Track width for a segmented control of `count` segments each `segment_w`
/// wide. Segments are seam to seam — the track's padding is the only gap.
pub(crate) fn segment_track_width(count: usize, segment_w: f32) -> f32 {
    count.max(1) as f32 * segment_w + SEGMENT_TRACK_PAD * 2.0
}

/// The segment rects inside `track`, left to right.
///
/// No gap between them by design: the seam is where a segmented control
/// differs from a row of buttons, and it is what makes the filled segment
/// read as a selection sliding along a trough rather than as one button
/// being on while its neighbour is off.
pub(crate) fn segment_rects(track: Rect, count: usize) -> Vec<Rect> {
    let count = count.max(1);
    let inner_w = (track.size.x - SEGMENT_TRACK_PAD * 2.0).max(0.0);
    let segment_w = inner_w / count as f32;
    (0..count)
        .map(|index| {
            Rect::xywh(
                track.origin.x + SEGMENT_TRACK_PAD + index as f32 * segment_w,
                track.origin.y + SEGMENT_TRACK_PAD,
                segment_w,
                (track.size.y - SEGMENT_TRACK_PAD * 2.0).max(0.0),
            )
        })
        .collect()
}

/// Segment width that fits the longest of `labels`, never below
/// [`SEGMENT_MIN_W`].
///
/// Equal widths, sized to the longest label: the selection then travels a
/// constant distance, and a group whose labels differ in length does not
/// look like it was laid out by accident.
///
/// The labels are read through `widths` in the chrome face they are
/// painted in (#440): paint measures them, the backend-less hit-test reads
/// the same widths, so both resolve one segment rect.
pub(crate) fn segment_width_for(widths: &mut LabelWidths<'_>, labels: &[&str]) -> f32 {
    labels
        .iter()
        .map(|label| widths.chrome(label, CHIP_LABEL_SIZE) + CHIP_PAD_X * 2.0)
        .fold(SEGMENT_MIN_W, f32::max)
}

/// Natural width of a [`paint_panel_chip`] holding `label`: the label as
/// painted (chrome face, [`CHIP_LABEL_SIZE`]) read through `widths`, plus
/// [`CHIP_PAD_X`] either side. Paint measures, the hit-test reads (#451).
pub(crate) fn chip_width_for(widths: &mut LabelWidths<'_>, label: &str) -> f32 {
    widths.chrome(label, CHIP_LABEL_SIZE) + CHIP_PAD_X * 2.0
}

/// One segment's pointer state, as the caller knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SegmentState {
    pub(crate) selected: bool,
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
}

/// Paint a segmented control: a low-contrast trough with one filled segment.
///
/// The contrast relationship is the whole control. The trough sits *below*
/// the panel surface so the group reads as inset; the selected segment sits
/// clearly above it. An unselected segment paints no fill at all — giving it
/// one would put three surfaces in a control whose entire job is to say
/// which of two things you are looking at.
pub(crate) fn paint_segmented_control(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    track: Rect,
    labels: &[&str],
    states: &[SegmentState],
) {
    cx.backend
        .fill_round_rect(track, track.size.y / 2.0, track_color(theme));
    for ((rect, label), state) in segment_rects(track, labels.len())
        .into_iter()
        .zip(labels.iter())
        .zip(states.iter())
    {
        let color = if state.selected {
            cx.backend.fill_round_rect(
                rect,
                CHIP_RADIUS,
                control_fill(theme.primary, state.hovered, state.pressed),
            );
            theme.primary_foreground
        } else {
            if state.hovered || state.pressed {
                // A wash rather than a surface: the unselected segment is
                // still part of the trough, and filling it would make the
                // control look like both segments were on.
                cx.backend
                    .fill_round_rect(rect, CHIP_RADIUS, theme.button_hover);
            }
            theme.muted_foreground
        };
        paint_centered_label(cx, rect, label, color);
    }
}

/// The trough behind a segmented control.
///
/// Derived from `muted` rather than named as its own token because it must
/// track the theme: a fixed grey that reads as "inset" on the dark popover
/// reads as "a box someone drew" on the light one.
fn track_color(theme: &Theme) -> Color {
    mix(theme.muted, theme.background, 0.35)
}

/// A filter/action chip. `selected` fills it in the accent colour; the rest
/// paint as a neutral surface with a hairline.
///
/// The hairline is what the old chips were missing. A filled `muted` pill on
/// a `popover` surface differs from its background by a few percent, so a
/// row of them read as text with a faint smudge behind it rather than as a
/// row of controls; the border is what turns each one back into an object.
pub(crate) fn paint_panel_chip(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    label: &str,
    state: SegmentState,
) {
    let color = if state.selected {
        cx.backend.fill_round_rect(
            rect,
            CHIP_RADIUS,
            control_fill(theme.primary, state.hovered, state.pressed),
        );
        theme.primary_foreground
    } else {
        cx.backend.fill_round_rect(
            rect,
            CHIP_RADIUS,
            control_fill(theme.secondary, state.hovered, state.pressed),
        );
        cx.backend
            .stroke_round_rect(rect, CHIP_RADIUS, theme.border, 1.0);
        if state.hovered || state.pressed {
            theme.foreground
        } else {
            theme.muted_foreground
        }
    };
    paint_centered_label(cx, rect, label, color);
}

/// Everything a labelled button needs beyond the theme it paints against.
///
/// One struct rather than six positional arguments: the two painters differ
/// only in weight, and a caller that swapped `hovered` and `pressed` between
/// them would compile and be wrong in a way nobody sees until they press.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ButtonSpec<'a> {
    pub(crate) rect: Rect,
    pub(crate) radius: f32,
    pub(crate) label: &'a str,
    pub(crate) label_size: f32,
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
}

/// A filled button carrying the panel's accent — the generate button, and a
/// card's primary action.
///
/// It rides the same [`control_fill`] ladder the segmented control's
/// selection does, which is the point: "the accent surface you can press"
/// is one thing in this panel, whatever shape it is cut to.
pub(crate) fn paint_accent_button(cx: &mut PaintCx<'_>, theme: &Theme, spec: ButtonSpec<'_>) {
    cx.backend.fill_round_rect(
        spec.rect,
        spec.radius,
        control_fill(theme.primary, spec.hovered, spec.pressed),
    );
    paint_label(
        cx,
        spec.rect,
        spec.label,
        spec.label_size,
        theme.primary_foreground,
    );
}

/// A bordered neutral button — the secondary half of a pair, weighted below
/// [`paint_accent_button`] so the primary reads as the default.
pub(crate) fn paint_neutral_button(cx: &mut PaintCx<'_>, theme: &Theme, spec: ButtonSpec<'_>) {
    cx.backend.fill_round_rect(
        spec.rect,
        spec.radius,
        control_fill(theme.card, spec.hovered, spec.pressed),
    );
    cx.backend
        .stroke_round_rect(spec.rect, spec.radius, theme.border, 1.0);
    paint_label(cx, spec.rect, spec.label, spec.label_size, theme.foreground);
}

fn paint_centered_label(cx: &mut PaintCx<'_>, rect: Rect, label: &str, color: Color) {
    paint_label(cx, rect, label, CHIP_LABEL_SIZE, color);
}

/// Paint `label` centred in `rect`, ellipsized when it does not fit. Both
/// the fit and the width are read through [`LabelWidths`], so a repaint
/// measures and allocates nothing.
fn paint_label(cx: &mut PaintCx<'_>, rect: Rect, label: &str, size: f32, color: Color) {
    let budget = (rect.size.x - LABEL_INSET * 2.0).max(0.0);
    let (label, width) = {
        let mut widths = LabelWidths::measuring(&mut *cx.backend);
        let label = widths.fit_chrome(label, budget, size);
        let width = widths.chrome(&label, size);
        (label, width)
    };
    let layout = TextLayout::single_run(
        &label,
        crate::widgets::text_metrics::CHROME_FONT_FAMILY,
        size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            rect.origin.x + ((rect.size.x - width) / 2.0).max(LABEL_INSET),
            jian_widgets::centered_text_baseline_y(rect, size),
        ),
    );
}

#[cfg(test)]
#[path = "panel_controls_tests.rs"]
mod panel_controls_tests;
