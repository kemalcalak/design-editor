use super::*;
use crate::vector_edit::parse_path_data;
use jian_ops_schema::style::StrokeThickness;

fn line() -> Vec<Subpath> {
    vec![Subpath::polyline(&[(0.0, 0.0), (100.0, 0.0)])]
}

fn ends(start: StrokeEndCap, end: StrokeEndCap) -> StrokeEnds {
    StrokeEnds {
        start,
        end,
        cap: StrokeEndCap::Round,
    }
}

/// [`stroke_end_geometry`] of a solid stroke.
fn solid(subs: &[Subpath], width: f64, e: StrokeEnds) -> Option<StrokeGeometry> {
    stroke_end_geometry(subs, width, e, CentreLine::Solid)
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
    // Both ends equal to an authored cap: the backends draw it already.
    assert_eq!(
        StrokeEnds::of(&stroke(
            Some(StrokeEndCap::Square),
            None,
            Some(StrokeCap::Square)
        )),
        None
    );
    // Flat ends on a round-capped stroke still need geometry.
    let flat = StrokeEnds::of(&stroke(
        Some(StrokeEndCap::None),
        Some(StrokeEndCap::None),
        Some(StrokeCap::Round),
    ))
    .expect("flat ends");
    assert_eq!(
        (flat.start, flat.end),
        (StrokeEndCap::None, StrokeEndCap::None)
    );
    let e = StrokeEnds::of(&stroke(Some(StrokeEndCap::Circle), None, None)).expect("ends");
    // No cap at all: open strokes paint round ends.
    assert_eq!(e.end, StrokeEndCap::Round);
}

#[test]
fn triangle_arrow_tip_sits_on_the_end_and_the_line_stops_under_its_base() {
    let g = solid(
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
    let g = solid(
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
    let g = solid(
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
        let g = solid(&line(), width, ends(StrokeEndCap::None, cap)).expect("geometry");
        let end = *points(&g.stroke_d).last().unwrap();
        let trim = 100.0 - end.0;
        assert!(trim >= 0.0 && trim <= head_length(width), "{cap:?}: {trim}");
        assert!(!g.fill_d.is_empty(), "{cap:?}");
    }
}

#[test]
fn round_and_square_caps_add_half_a_width_beyond_the_end() {
    let g = solid(
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
    assert!(g.fill_d.contains(" A 2 2 0 0 1 102 0"), "{}", g.fill_d);
    assert_eq!(points(&g.stroke_d), vec![(0.0, 0.0), (100.0, 0.0)]);
}

#[test]
fn curves_keep_their_tangent_and_trim_along_the_curve() {
    let subs = parse_path_data("M0 0 C 0 50 50 100 100 100").expect("path");
    let g = solid(
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
    assert_eq!(solid(&closed, 2.0, e), None);
    assert_eq!(solid(&line(), 0.0, e), None);
    // Flat ends draw no shape but still a butt-capped line.
    let flat = solid(&line(), 2.0, ends(StrokeEndCap::None, StrokeEndCap::None)).expect("flat");
    assert!(flat.fill_d.is_empty() && flat.arms_d.is_empty());
    assert_eq!(points(&flat.stroke_d), vec![(0.0, 0.0), (100.0, 0.0)]);
    let dot = vec![Subpath::polyline(&[(5.0, 5.0), (5.0, 5.0)])];
    assert_eq!(solid(&dot, 2.0, e), None);
}

#[test]
fn a_head_longer_than_the_line_keeps_a_sliver_of_it() {
    let short = vec![Subpath::polyline(&[(0.0, 0.0), (3.0, 0.0)])];
    let g = solid(
        &short,
        2.0,
        ends(StrokeEndCap::TriangleArrow, StrokeEndCap::TriangleArrow),
    )
    .expect("geometry");
    assert!(!g.stroke_d.is_empty());
}

const ALL_ENDS: [StrokeEndCap; 8] = [
    StrokeEndCap::None,
    StrokeEndCap::Round,
    StrokeEndCap::Square,
    StrokeEndCap::LineArrow,
    StrokeEndCap::TriangleArrow,
    StrokeEndCap::ReversedTriangle,
    StrokeEndCap::Circle,
    StrokeEndCap::Diamond,
];

#[test]
fn a_dotted_line_keeps_its_dots_and_stops_half_a_width_short_of_every_end() {
    // Dotted = zero-length dashes with a round cap: the centre line is
    // drawn with that cap, so it must never be empty, and it ends half a
    // width before each end (or before the head's trim) so no dot pokes
    // past it.
    let width = 4.0;
    for end in ALL_ENDS {
        let g = stroke_end_geometry(
            &line(),
            width,
            ends(StrokeEndCap::None, end),
            CentreLine::Dashed,
        )
        .expect("geometry");
        let pts = points(&g.stroke_d);
        assert_eq!(pts.first(), Some(&(2.0, 0.0)), "{end:?}: {}", g.stroke_d);
        let solid_end = points(
            &solid(&line(), width, ends(StrokeEndCap::None, end))
                .unwrap()
                .stroke_d,
        )
        .last()
        .copied()
        .unwrap();
        let dashed_end = *pts.last().unwrap();
        assert!(
            (solid_end.0 - dashed_end.0 - width / 2.0).abs() < 1e-6,
            "{end:?}: solid {solid_end:?}, dashed {dashed_end:?}"
        );
    }
}

#[test]
fn butt_dashes_are_not_shortened() {
    let e = StrokeEnds {
        cap: StrokeEndCap::None,
        ..ends(StrokeEndCap::None, StrokeEndCap::None)
    };
    let g = stroke_end_geometry(&line(), 4.0, e, CentreLine::Dashed).expect("geometry");
    assert_eq!(points(&g.stroke_d), vec![(0.0, 0.0), (100.0, 0.0)]);
}

#[test]
fn inner_ends_of_a_multi_subpath_stroke_keep_the_cap() {
    let subs = parse_path_data("M0 0 L40 0 M0 20 L40 20 M0 40 L40 40").expect("path");
    let e = ends(StrokeEndCap::TriangleArrow, StrokeEndCap::TriangleArrow);
    let g = solid(&subs, 2.0, e).expect("geometry");
    // Two heads plus a round cap on each of the four inner ends.
    assert_eq!(g.fill_d.matches(" A ").count(), 4 * 2, "{}", g.fill_d);
    assert_eq!(g.heads.len(), 2);
    // The middle subpath is stroked whole.
    assert!(g.stroke_d.contains("M 0 20 L 40 20"), "{}", g.stroke_d);

    // Dashed: the dashes carry the cap, so no extra shapes, and the inner
    // ends are not shortened.
    let dashed = stroke_end_geometry(&subs, 2.0, e, CentreLine::Dashed).expect("geometry");
    assert!(!dashed.fill_d.contains(" A "), "{}", dashed.fill_d);
    assert!(
        dashed.stroke_d.contains("M 0 20 L 40 20"),
        "{}",
        dashed.stroke_d
    );
    assert!(dashed.stroke_d.contains("L 40 0"), "{}", dashed.stroke_d);
}

#[test]
fn every_filled_shape_winds_the_same_way() {
    // Signed area (y down) of each straight-edged subpath of `d`.
    fn areas(d: &str) -> Vec<f64> {
        d.split('M')
            .filter(|part| !part.trim().is_empty() && !part.contains('A'))
            .map(|part| {
                let pts = points(&format!("M{part}"));
                let mut area = 0.0;
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                    area += a.0 * b.1 - b.0 * a.1;
                }
                area
            })
            .collect()
    }
    for start in ALL_ENDS {
        for end in ALL_ENDS {
            let g = solid(&line(), 3.0, ends(start, end)).expect("geometry");
            for area in areas(&g.fill_d) {
                assert!(area > 0.0, "{start:?} / {end:?}: {}", g.fill_d);
            }
            // Arcs turn the same way as the polygons (sweep 1).
            assert!(
                !g.fill_d.contains(" 0 0 0 "),
                "{start:?} / {end:?}: {}",
                g.fill_d
            );
        }
    }
}

#[test]
fn markers_report_a_disc_that_covers_them() {
    let width = 2.0;
    let s = head_length(width);
    for end in ALL_ENDS {
        let g = solid(&line(), width, ends(StrokeEndCap::None, end)).expect("geometry");
        // Arc radii and flags are not points: read each arc as its end.
        let mut shapes = String::new();
        let mut skip = 0;
        for token in format!("{} {}", g.fill_d, g.arms_d).split_whitespace() {
            if skip > 0 {
                skip -= 1;
            } else if token == "A" {
                shapes.push_str(" L");
                skip = 5;
            } else {
                shapes.push(' ');
                shapes.push_str(token);
            }
        }
        if matches!(
            end,
            StrokeEndCap::None | StrokeEndCap::Round | StrokeEndCap::Square
        ) {
            assert!(g.heads.is_empty(), "{end:?}");
            continue;
        }
        let [head] = g.heads[..] else {
            panic!("{end:?}: {:?}", g.heads);
        };
        for (x, y) in points(&shapes) {
            let reach = (x - head.center.0).hypot(y - head.center.1);
            assert!(reach <= head.radius, "{end:?}: {x},{y} outside {head:?}");
        }
        assert!(head.radius <= s + width, "{end:?}");
    }
}
