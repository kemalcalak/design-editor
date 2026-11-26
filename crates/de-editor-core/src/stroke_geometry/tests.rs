use super::*;
use crate::vector_edit::parse_path_data;
use jian_ops_schema::style::StrokeThickness;

fn line() -> Vec<Subpath> {
    vec![Subpath::polyline(&[(0.0, 0.0), (100.0, 0.0)])]
}

fn ends(start: StrokeEndCap, end: StrokeEndCap) -> StrokeEnds {
    StrokeEnds { start, end }
}

fn stroke(
    start: Option<StrokeEndCap>,
    end: Option<StrokeEndCap>,
    cap: Option<StrokeCap>,
) -> PenStroke {
    PenStroke {
        thickness: StrokeThickness::Uniform(2.0),
        align: None,
        join: None,
        cap,
        dash_pattern: None,
        dash_offset: None,
        fill: None,
        stroke_start: start,
        stroke_end: end,
        miter_limit: None,
        width_profile: None,
    }
}

/// Every coordinate pair of an SVG path written by this module.
fn points(d: &str) -> Vec<(f64, f64)> {
    let nums: Vec<f64> = d
        .split(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse().unwrap())
        .collect();
    nums.chunks(2).map(|c| (c[0], c[1])).collect()
}

#[test]
fn unset_ends_fall_back_to_cap_and_no_ends_means_no_geometry() {
    assert_eq!(
        StrokeEnds::of(&stroke(None, None, Some(StrokeCap::Round))),
        None
    );
    let e = StrokeEnds::of(&stroke(
        None,
        Some(StrokeEndCap::TriangleArrow),
        Some(StrokeCap::Round),
    ))
    .expect("ends");
    assert_eq!(e, ends(StrokeEndCap::Round, StrokeEndCap::TriangleArrow));
    let e = StrokeEnds::of(&stroke(Some(StrokeEndCap::Circle), None, None)).expect("ends");
    // No cap at all: open strokes paint round ends.
    assert_eq!(e.end, StrokeEndCap::Round);
}

#[test]
fn triangle_arrow_tip_sits_on_the_end_and_the_line_stops_under_its_base() {
    let g = stroke_end_geometry(
        &line(),
        2.0,
        ends(StrokeEndCap::None, StrokeEndCap::TriangleArrow),
    )
    .expect("geometry");
    let s = head_length(2.0);
    assert_eq!(s, 10.0);
    let head = points(&g.fill_d);
    assert!(head.contains(&(100.0, 0.0)), "{}", g.fill_d);
    assert!(
        head.contains(&(90.0, 5.0)) && head.contains(&(90.0, -5.0)),
        "{}",
        g.fill_d
    );
    let line = points(&g.stroke_d);
    assert_eq!(line.first(), Some(&(0.0, 0.0)));
    let end = line.last().unwrap();
    assert!((end.0 - 90.5).abs() < 1e-6, "{}", g.stroke_d);
    assert!(
        g.stroke_d.contains('L') && !g.stroke_d.contains('C'),
        "{}",
        g.stroke_d
    );
    assert!(g.arms_d.is_empty());
}

#[test]
fn start_heads_point_away_from_the_path() {
    let g = stroke_end_geometry(
        &line(),
        2.0,
        ends(StrokeEndCap::TriangleArrow, StrokeEndCap::None),
    )
    .expect("geometry");
    let head = points(&g.fill_d);
    assert!(
        head.contains(&(0.0, 0.0)) && head.contains(&(10.0, 5.0)),
        "{}",
        g.fill_d
    );
    assert_eq!(points(&g.stroke_d).last(), Some(&(100.0, 0.0)));
}

#[test]
fn line_arrow_arms_meet_at_the_tip_without_trimming() {
    let g = stroke_end_geometry(
        &line(),
        1.0,
        ends(StrokeEndCap::None, StrokeEndCap::LineArrow),
    )
    .expect("geometry");
    assert!(g.fill_d.is_empty());
    let arms = points(&g.arms_d);
    assert_eq!(arms.len(), 3);
    assert_eq!(arms[1], (100.0, 0.0));
    assert!(arms[0].0 < 100.0 && arms[0].1 > 0.0 && arms[2].1 < 0.0);
    assert_eq!(points(&g.stroke_d).last(), Some(&(100.0, 0.0)));
}

#[test]
fn every_marker_covers_the_trimmed_line_end() {
    for cap in [
        StrokeEndCap::ReversedTriangle,
        StrokeEndCap::Circle,
        StrokeEndCap::Diamond,
        StrokeEndCap::TriangleArrow,
    ] {
        let width = 4.0;
        let g =
            stroke_end_geometry(&line(), width, ends(StrokeEndCap::None, cap)).expect("geometry");
        let end = *points(&g.stroke_d).last().unwrap();
        let trim = 100.0 - end.0;
        assert!(trim >= 0.0 && trim <= head_length(width), "{cap:?}: {trim}");
        assert!(!g.fill_d.is_empty(), "{cap:?}");
    }
}

#[test]
fn round_and_square_caps_add_half_a_width_beyond_the_end() {
    let g = stroke_end_geometry(
        &line(),
        4.0,
        ends(StrokeEndCap::Square, StrokeEndCap::Round),
    )
    .expect("geometry");
    let pts = points(&g.fill_d);
    assert!(
        pts.contains(&(-2.0, 2.0)) && pts.contains(&(-2.0, -2.0)),
        "{}",
        g.fill_d
    );
    assert!(g.fill_d.contains(" A 2 2 0 0 0 102 0"), "{}", g.fill_d);
    assert_eq!(points(&g.stroke_d), vec![(0.0, 0.0), (100.0, 0.0)]);
}

#[test]
fn curves_keep_their_tangent_and_trim_along_the_curve() {
    let subs = parse_path_data("M0 0 C 0 50 50 100 100 100").expect("path");
    let g = stroke_end_geometry(
        &subs,
        2.0,
        ends(StrokeEndCap::None, StrokeEndCap::TriangleArrow),
    )
    .expect("geometry");
    // The end tangent is +x, so the base corners sit 10 px to the left.
    let head = points(&g.fill_d);
    assert!(
        head.contains(&(100.0, 100.0)) && head.contains(&(90.0, 105.0)),
        "{}",
        g.fill_d
    );
    assert!(g.stroke_d.contains('C'), "{}", g.stroke_d);
    let end = *points(&g.stroke_d).last().unwrap();
    let gap = (100.0 - end.0).hypot(100.0 - end.1);
    assert!((gap - 9.5).abs() < 0.1, "{end:?}");
}

#[test]
fn closed_paths_and_degenerate_input_have_no_ends() {
    let closed = parse_path_data("M0 0 L10 0 L10 10 Z").expect("path");
    let e = ends(StrokeEndCap::TriangleArrow, StrokeEndCap::TriangleArrow);
    assert_eq!(stroke_end_geometry(&closed, 2.0, e), None);
    assert_eq!(stroke_end_geometry(&line(), 0.0, e), None);
    assert_eq!(
        stroke_end_geometry(&line(), 2.0, ends(StrokeEndCap::None, StrokeEndCap::None)),
        None
    );
    let dot = vec![Subpath::polyline(&[(5.0, 5.0), (5.0, 5.0)])];
    assert_eq!(stroke_end_geometry(&dot, 2.0, e), None);
}

#[test]
fn a_head_longer_than_the_line_keeps_a_sliver_of_it() {
    let short = vec![Subpath::polyline(&[(0.0, 0.0), (3.0, 0.0)])];
    let g = stroke_end_geometry(
        &short,
        2.0,
        ends(StrokeEndCap::TriangleArrow, StrokeEndCap::TriangleArrow),
    )
    .expect("geometry");
    assert!(!g.stroke_d.is_empty());
}
