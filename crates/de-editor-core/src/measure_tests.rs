use super::*;

fn r(x: f64, y: f64, w: f64, h: f64) -> DocRect {
    DocRect { x, y, w, h }
}

fn values(m: &Measurement) -> Vec<f64> {
    m.lines.iter().map(|l| l.value).collect()
}

#[test]
fn parent_shows_the_padding_on_all_four_sides() {
    // A 40x20 child at (10, 30) inside a 100x100 frame at the origin.
    let m = measure(r(10.0, 30.0, 40.0, 20.0), r(0.0, 0.0, 100.0, 100.0));
    assert_eq!(values(&m), vec![10.0, 50.0, 30.0, 50.0]);
    // Horizontal lines run through the child's centre row.
    assert_eq!(m.lines[0].from, (0.0, 40.0));
    assert_eq!(m.lines[0].to, (10.0, 40.0));
    assert!(m.lines[2].vertical());
    assert!(m.guides.is_empty());
}

#[test]
fn a_child_flush_with_an_edge_leaves_that_side_out() {
    let m = measure(r(0.0, 0.0, 40.0, 20.0), r(0.0, 0.0, 100.0, 100.0));
    assert_eq!(values(&m), vec![60.0, 80.0]);
}

#[test]
fn hovering_a_child_of_the_selection_inverts_the_padding() {
    let m = measure(r(0.0, 0.0, 100.0, 100.0), r(10.0, 30.0, 40.0, 20.0));
    assert_eq!(values(&m), vec![10.0, 50.0, 30.0, 50.0]);
}

#[test]
fn siblings_side_by_side_measure_the_gap_through_the_shared_band() {
    // Selection 0..50 x 0..40, target 70..120 x 20..80: gap 20 on x,
    // vertical overlap 20..40 so the line runs at y = 30.
    let m = measure(r(0.0, 0.0, 50.0, 40.0), r(70.0, 20.0, 50.0, 60.0));
    assert_eq!(values(&m), vec![20.0]);
    assert_eq!(m.lines[0].from, (50.0, 30.0));
    assert_eq!(m.lines[0].to, (70.0, 30.0));
    assert!(m.guides.is_empty());
}

#[test]
fn a_target_above_measures_the_vertical_gap() {
    let m = measure(r(0.0, 100.0, 50.0, 40.0), r(10.0, 0.0, 20.0, 64.5));
    assert_eq!(values(&m), vec![35.5]);
    assert!(m.lines[0].vertical());
    // Through the middle of the shared columns 10..30.
    assert_eq!(m.lines[0].from.0, 20.0);
}

#[test]
fn a_diagonal_target_gets_both_gaps_and_guides() {
    // Target down and to the right, no overlap on either axis.
    let m = measure(r(0.0, 0.0, 10.0, 10.0), r(30.0, 50.0, 10.0, 10.0));
    assert_eq!(values(&m), vec![20.0, 40.0]);
    assert_eq!(m.guides.len(), 2);
    // The horizontal gap runs at the selection's centre row and the
    // guide drops from its far end to the target's top edge.
    assert_eq!(m.lines[0].from, (10.0, 5.0));
    assert_eq!(m.guides[0].from, (30.0, 5.0));
    assert_eq!(m.guides[0].to, (30.0, 50.0));
}

#[test]
fn overlapping_boxes_measure_matching_edges() {
    let m = measure(r(0.0, 0.0, 50.0, 50.0), r(20.0, 10.0, 50.0, 50.0));
    assert_eq!(values(&m), vec![20.0, 20.0, 10.0, 10.0]);
}

#[test]
fn identical_boxes_have_nothing_to_measure() {
    let m = measure(r(5.0, 5.0, 10.0, 10.0), r(5.0, 5.0, 10.0, 10.0));
    assert!(m.lines.is_empty());
    assert!(m.guides.is_empty());
}

#[test]
fn non_finite_input_measures_nothing() {
    let m = measure(r(f64::NAN, 0.0, 1.0, 1.0), r(0.0, 0.0, 10.0, 10.0));
    assert_eq!(m, Measurement::default());
}

#[test]
fn values_format_with_at_most_two_decimals() {
    assert_eq!(format_measure_value(12.0), "12");
    assert_eq!(format_measure_value(12.5), "12.5");
    assert_eq!(format_measure_value(1.0 / 3.0), "0.33");
    assert_eq!(format_measure_value(0.999), "1");
    assert_eq!(format_measure_value(120.0), "120");
    assert_eq!(format_measure_value(-0.001), "0");
}
