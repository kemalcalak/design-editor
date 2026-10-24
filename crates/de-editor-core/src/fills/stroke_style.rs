//! Stroke *style* helpers: everything about a stroke beyond its colour
//! and widths — alignment (#90), caps, joins and dashes (#89).
//!
//! Every writer edits an existing stroke only: a node without one is
//! left alone (no invisible stroke is created just to carry a style),
//! and each returns whether the document changed.

use super::*;
use jian_ops_schema::style::{StrokeAlign, StrokeCap, StrokeJoin};

/// The node's stroke alignment, `None` when it has no stroke. An absent
/// `align` field reads as `Center`, which is how every renderer treats it.
pub fn node_stroke_align(node: &PenNode) -> Option<StrokeAlign> {
    node_stroke(node).map(|stroke| stroke.align.clone().unwrap_or(StrokeAlign::Center))
}

/// Write the alignment of an existing stroke. Returns `false` when the
/// node has no stroke or the alignment is already `align`.
pub fn set_stroke_align(node: &mut PenNode, align: StrokeAlign) -> bool {
    let Some(Some(stroke)) = node_stroke_mut(node) else {
        return false;
    };
    if stroke.align.as_ref().unwrap_or(&StrokeAlign::Center) == &align {
        return false;
    }
    stroke.align = Some(align);
    true
}

/// Dash presets the Stroke section offers. Lengths scale with the
/// stroke width so a preset reads the same at 1 px and at 8 px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeDashPreset {
    Solid,
    /// `[4w, 4w]`.
    Dashed,
    /// `[0, 2w]` with round caps: a zero-length dash is a round dot.
    Dotted,
}

impl StrokeDashPreset {
    pub const ALL: [StrokeDashPreset; 3] = [Self::Solid, Self::Dashed, Self::Dotted];

    /// The pattern this preset writes for a stroke `width` px wide
    /// (`None` = solid).
    pub fn pattern(self, width: f32) -> Option<Vec<f32>> {
        let w = preset_unit(width);
        match self {
            Self::Solid => None,
            Self::Dashed => Some(vec![4.0 * w, 4.0 * w]),
            Self::Dotted => Some(vec![0.0, 2.0 * w]),
        }
    }

    /// The preset an authored pattern matches at `width`, `None` for a
    /// custom pattern (an import, an MCP write) no preset reproduces.
    pub fn matching(pattern: Option<&[f32]>, width: f32) -> Option<Self> {
        let pattern = pattern.filter(|p| !p.is_empty());
        Self::ALL
            .into_iter()
            .find(|preset| match (preset.pattern(width).as_deref(), pattern) {
                (None, None) => true,
                (Some(want), Some(have)) => {
                    want.len() == have.len()
                        && want.iter().zip(have).all(|(a, b)| (a - b).abs() < 0.01)
                }
                _ => false,
            })
    }
}

/// Preset lengths are multiples of the stroke width, floored at 1 px so
/// a hairline still gets a visible pattern.
fn preset_unit(width: f32) -> f32 {
    if width.is_finite() {
        width.max(1.0)
    } else {
        1.0
    }
}

/// The node's authored stroke cap / join / dash, `None` when it has no
/// stroke. Absent fields stay `None`: renderers keep their per-primitive
/// defaults for them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeStyleFields {
    pub cap: Option<StrokeCap>,
    pub join: Option<StrokeJoin>,
    pub dash_pattern: Option<Vec<f32>>,
    pub dash_offset: Option<f32>,
}

pub fn node_stroke_style(node: &PenNode) -> Option<StrokeStyleFields> {
    node_stroke(node).map(|stroke| StrokeStyleFields {
        cap: stroke.cap.clone(),
        join: stroke.join.clone(),
        dash_pattern: stroke.dash_pattern.clone(),
        dash_offset: stroke.dash_offset,
    })
}

/// The render-side style (jian's `StrokeStyle`, carried on `SceneStroke`)
/// of a schema stroke's cap / join / dash. Unset fields stay unset, so a
/// plain stroke paints with the backends' defaults; a dash nothing can
/// draw reads as solid (`StrokeDash::new`).
pub fn render_stroke_style(
    cap: Option<&StrokeCap>,
    join: Option<&StrokeJoin>,
    dash_pattern: Option<&[f32]>,
    dash_offset: Option<f32>,
) -> jian_widgets::StrokeStyle {
    use jian_widgets::{StrokeCap as Cap, StrokeDash, StrokeJoin as Join};
    jian_widgets::StrokeStyle {
        cap: cap.map(|cap| match cap {
            StrokeCap::None => Cap::Butt,
            StrokeCap::Round => Cap::Round,
            StrokeCap::Square => Cap::Square,
        }),
        join: join.map(|join| match join {
            StrokeJoin::Miter => Join::Miter,
            StrokeJoin::Round => Join::Round,
            StrokeJoin::Bevel => Join::Bevel,
        }),
        dash: dash_pattern.and_then(|pattern| StrokeDash::new(pattern, dash_offset.unwrap_or(0.0))),
    }
}

/// [`render_stroke_style`] of a whole schema stroke.
pub fn pen_stroke_render_style(stroke: &PenStroke) -> jian_widgets::StrokeStyle {
    render_stroke_style(
        stroke.cap.as_ref(),
        stroke.join.as_ref(),
        stroke.dash_pattern.as_deref(),
        stroke.dash_offset,
    )
}

/// A partial write of a stroke's cap / join / dash. `None` fields are
/// left untouched; an empty `dash_pattern` clears the dash (solid).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeStyleWrite {
    pub cap: Option<StrokeCap>,
    pub join: Option<StrokeJoin>,
    pub dash_pattern: Option<Vec<f32>>,
    pub dash_offset: Option<f32>,
}

impl StrokeStyleWrite {
    pub fn is_empty(&self) -> bool {
        self.cap.is_none()
            && self.join.is_none()
            && self.dash_pattern.is_none()
            && self.dash_offset.is_none()
    }

    /// Reject what no renderer can draw: a negative or non-finite dash
    /// length, a pattern whose lengths sum to zero, a non-finite offset.
    pub fn is_valid(&self) -> bool {
        let pattern_ok = self.dash_pattern.as_ref().is_none_or(|pattern| {
            pattern.is_empty()
                || (pattern.iter().all(|v| v.is_finite() && *v >= 0.0)
                    && pattern.iter().sum::<f32>() > 0.0)
        });
        pattern_ok && self.dash_offset.is_none_or(f32::is_finite)
    }
}

/// Apply a partial style write to an existing stroke. Returns `false`
/// when the node has no stroke, the write is invalid, or nothing changed.
pub fn set_stroke_style(node: &mut PenNode, write: &StrokeStyleWrite) -> bool {
    if !write.is_valid() {
        return false;
    }
    let Some(Some(stroke)) = node_stroke_mut(node) else {
        return false;
    };
    let before = (
        stroke.cap.clone(),
        stroke.join.clone(),
        stroke.dash_pattern.clone(),
        stroke.dash_offset,
    );
    if let Some(cap) = &write.cap {
        stroke.cap = Some(cap.clone());
    }
    if let Some(join) = &write.join {
        stroke.join = Some(join.clone());
    }
    if let Some(pattern) = &write.dash_pattern {
        stroke.dash_pattern = (!pattern.is_empty()).then(|| pattern.clone());
    }
    if let Some(offset) = write.dash_offset {
        stroke.dash_offset = (offset != 0.0).then_some(offset);
    }
    if stroke.dash_pattern.is_none() {
        // A solid stroke has no phase to keep.
        stroke.dash_offset = None;
    }
    before
        != (
            stroke.cap.clone(),
            stroke.join.clone(),
            stroke.dash_pattern.clone(),
            stroke.dash_offset,
        )
}

/// Apply a dash preset at the stroke's current width. Dotted also sets
/// round caps, without which a zero-length dash paints nothing.
pub fn set_stroke_dash_preset(node: &mut PenNode, preset: StrokeDashPreset) -> bool {
    let Some(width) = node_stroke_width(node) else {
        return false;
    };
    let write = StrokeStyleWrite {
        cap: (preset == StrokeDashPreset::Dotted).then_some(StrokeCap::Round),
        dash_pattern: Some(preset.pattern(width as f32).unwrap_or_default()),
        // A preset starts at phase 0, whatever offset an import carried.
        dash_offset: Some(0.0),
        ..StrokeStyleWrite::default()
    };
    set_stroke_style(node, &write)
}

/// One click in the Stroke section's cap / join / dash rows.
#[derive(Debug, Clone, PartialEq)]
pub enum StrokeStyleEdit {
    Cap(StrokeCap),
    Join(StrokeJoin),
    Dash(StrokeDashPreset),
}

pub fn apply_stroke_style_edit(node: &mut PenNode, edit: &StrokeStyleEdit) -> bool {
    match edit {
        StrokeStyleEdit::Cap(cap) => set_stroke_style(
            node,
            &StrokeStyleWrite {
                cap: Some(cap.clone()),
                ..StrokeStyleWrite::default()
            },
        ),
        StrokeStyleEdit::Join(join) => set_stroke_style(
            node,
            &StrokeStyleWrite {
                join: Some(join.clone()),
                ..StrokeStyleWrite::default()
            },
        ),
        StrokeStyleEdit::Dash(preset) => set_stroke_dash_preset(node, *preset),
    }
}

#[cfg(test)]
#[path = "stroke_style_tests.rs"]
mod tests;
