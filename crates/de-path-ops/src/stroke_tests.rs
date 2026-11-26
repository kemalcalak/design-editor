use super::*;
use crate::test_raster::{coverage, covered};
use de_editor_core::render_backend::Rect;
use jian_scene::layout_scene::{NodeKind, SceneStroke, StrokeDash};

fn stroked_rect(align: SceneStrokeAlign, style: StrokeStyle) -> SceneNode {
    let mut n = SceneNode::leaf("r", NodeKind::Rect);
    n.bounds = Rect::xywh(20.0, 20.0, 40.0, 40.0);
    n.stroke = Some(SceneStroke {
        color: de_editor_core::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        width: 4.0,
        sides: None,
        align,
        style,
    });
    n
}

#[test]
fn outline_stroke_honours_alignment() {
    // A 40 px square with a 4 px stroke paints a 4 px band: inside it
    // is 40² − 32², centred 44² − 36², outside 48² − 40².
    let cases = [
        (SceneStrokeAlign::Inside, 1600 - 1024),
        (SceneStrokeAlign::Center, 1936 - 1296),
        (SceneStrokeAlign::Outside, 2304 - 1600),
    ];
    for (align, expected) in cases {
        let band = outline_stroke(&stroked_rect(align, StrokeStyle::default())).unwrap();
        let px = coverage(&band, 80, 80) as i64;
        assert!(
            (px - expected).abs() <= 8,
            "{align:?}: {px} px, expected {expected}"
        );
        assert!(!covered(&band, 40.0, 40.0), "the middle stays empty");
    }
}

#[test]
fn outline_stroke_honours_dash() {
    let solid = outline_stroke(&stroked_rect(
        SceneStrokeAlign::Center,
        StrokeStyle::default(),
    ));
    let dashed = outline_stroke(&stroked_rect(
        SceneStrokeAlign::Center,
        StrokeStyle {
            dash: StrokeDash::new(&[8.0, 8.0], 0.0),
            ..StrokeStyle::default()
        },
    ));
    let (solid, dashed) = (
        coverage(&solid.unwrap(), 80, 80),
        coverage(&dashed.unwrap(), 80, 80),
    );
    assert!(
        dashed > solid * 4 / 10 && dashed < solid * 6 / 10,
        "dashed {dashed} vs solid {solid}"
    );
}

#[test]
fn round_cap_extends_an_open_stroke() {
    let mut line = SceneNode::leaf("l", NodeKind::Line);
    line.points = vec![
        de_editor_core::render_backend::Point2D::new(10.0, 10.0),
        de_editor_core::render_backend::Point2D::new(50.0, 10.0),
    ];
    line.stroke = Some(SceneStroke {
        color: de_editor_core::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        width: 6.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: StrokeStyle {
            cap: Some(StrokeCap::Round),
            ..StrokeStyle::default()
        },
    });
    let b = outline_stroke(&line).unwrap().compute_tight_bounds();
    assert!((b.left() - 7.0).abs() < 0.01 && (b.right() - 53.0).abs() < 0.01);
}

#[test]
fn no_stroke_means_no_outline() {
    let mut n = stroked_rect(SceneStrokeAlign::Center, StrokeStyle::default());
    n.stroke = None;
    assert!(outline_stroke(&n).is_none());
}

fn square(x: f32, y: f32, s: f32) -> Path {
    let mut b = PathBuilder::new();
    b.add_rect(skia_safe::Rect::from_xywh(x, y, s, s), None, None);
    b.detach()
}

#[test]
fn offset_grows_and_shrinks_by_delta() {
    let sq = square(20.0, 20.0, 40.0);
    let grown = offset_path(&sq, 5.0, OffsetJoin::Miter).unwrap();
    let b = grown.compute_tight_bounds();
    assert!(
        (b.width() - 50.0).abs() < 0.01,
        "miter keeps square corners"
    );
    assert_eq!(coverage(&grown, 80, 80), 2500);
    let shrunk = offset_path(&sq, -5.0, OffsetJoin::Miter).unwrap();
    assert_eq!(coverage(&shrunk, 80, 80), 900);
    assert!(
        offset_path(&sq, -25.0, OffsetJoin::Round).is_none(),
        "inset past the middle"
    );
}

#[test]
fn offset_joins_differ_at_corners() {
    let sq = square(20.0, 20.0, 40.0);
    let px = |join| coverage(&offset_path(&sq, 8.0, join).unwrap(), 80, 80);
    let (miter, round, bevel) = (
        px(OffsetJoin::Miter),
        px(OffsetJoin::Round),
        px(OffsetJoin::Bevel),
    );
    assert!(
        miter > round && round > bevel,
        "{miter} > {round} > {bevel}"
    );
}

#[test]
fn join_names_parse() {
    assert_eq!(OffsetJoin::parse("Bevel"), Some(OffsetJoin::Bevel));
    assert_eq!(OffsetJoin::parse("x"), None);
}
