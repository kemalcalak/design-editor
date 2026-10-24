//! Stroke *style* helpers: everything about a stroke beyond its colour
//! and widths. Alignment lives here today; caps, joins and dashes (#89)
//! belong next to it.

use super::*;
use jian_ops_schema::style::StrokeAlign;

/// The node's stroke alignment, `None` when it has no stroke. An absent
/// `align` field reads as `Center`, which is how every renderer treats it.
pub fn node_stroke_align(node: &PenNode) -> Option<StrokeAlign> {
    node_stroke(node).map(|stroke| stroke.align.clone().unwrap_or(StrokeAlign::Center))
}

/// Write the alignment of an existing stroke. Returns `false` when the
/// node has no stroke (no invisible stroke is created just to carry an
/// alignment) or the alignment is already `align`.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(stroke: &str) -> PenNode {
        let src = format!(r#"{{"type":"rectangle","id":"r1","width":10,"height":10{stroke}}}"#);
        serde_json::from_str(&src).expect("rect fixture")
    }

    #[test]
    fn missing_align_reads_center_and_no_stroke_reads_none() {
        assert_eq!(node_stroke_align(&rect("")), None);
        let stroked = rect(r#","stroke":{"thickness":2}"#);
        assert_eq!(node_stroke_align(&stroked), Some(StrokeAlign::Center));
    }

    #[test]
    fn set_stroke_align_writes_existing_strokes_only() {
        let mut bare = rect("");
        assert!(!set_stroke_align(&mut bare, StrokeAlign::Inside));
        assert_eq!(node_stroke_align(&bare), None);

        let mut stroked = rect(r#","stroke":{"thickness":2}"#);
        assert!(!set_stroke_align(&mut stroked, StrokeAlign::Center));
        assert!(set_stroke_align(&mut stroked, StrokeAlign::Outside));
        assert_eq!(node_stroke_align(&stroked), Some(StrokeAlign::Outside));
        assert!(!set_stroke_align(&mut stroked, StrokeAlign::Outside));
    }
}
