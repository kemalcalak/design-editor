use super::*;
use crate::geometry::DocRect;

fn rect(x: f64, y: f64, w: f64, h: f64) -> DocRect {
    DocRect { x, y, w, h }
}

fn positioned(x: f64, y: f64, w: f64, h: f64) -> IncomingRoot {
    IncomingRoot {
        bounds: rect(x, y, w, h),
        positioned: true,
    }
}

fn unpositioned(w: f64, h: f64) -> IncomingRoot {
    IncomingRoot {
        bounds: rect(0.0, 0.0, w, h),
        positioned: false,
    }
}

#[test]
fn positioned_root_over_existing_content_moves_right_of_it() {
    let existing = [rect(0.0, 0.0, 1200.0, 800.0)];
    let plan = plan_root_placement(&existing, &[positioned(200.0, 200.0, 400.0, 300.0)], false);
    assert_eq!(plan, vec![Some((1280.0, 0.0))]);
}

#[test]
fn positioned_root_in_free_space_stays() {
    let existing = [rect(0.0, 0.0, 1200.0, 800.0)];
    let plan = plan_root_placement(&existing, &[positioned(0.0, 900.0, 400.0, 300.0)], false);
    assert_eq!(plan, vec![None]);
}

#[test]
fn touching_edges_do_not_count_as_overlap() {
    let existing = [rect(0.0, 0.0, 100.0, 100.0)];
    let plan = plan_root_placement(&existing, &[positioned(100.0, 0.0, 50.0, 50.0)], false);
    assert_eq!(plan, vec![None]);
}

#[test]
fn allow_overlap_keeps_a_positioned_root() {
    let existing = [rect(0.0, 0.0, 1200.0, 800.0)];
    let plan = plan_root_placement(&existing, &[positioned(200.0, 200.0, 400.0, 300.0)], true);
    assert_eq!(plan, vec![None]);
}

#[test]
fn unpositioned_root_on_an_empty_page_stays_at_origin() {
    let plan = plan_root_placement(&[], &[unpositioned(390.0, 844.0)], false);
    assert_eq!(plan, vec![None]);
}

#[test]
fn unpositioned_root_goes_right_of_content_even_when_free() {
    // Content starts at x=600, so (0,0) would not overlap, but a root with
    // no position still joins the row on the right.
    let existing = [rect(600.0, 40.0, 390.0, 844.0)];
    let plan = plan_root_placement(&existing, &[unpositioned(390.0, 844.0)], false);
    assert_eq!(plan, vec![Some((1070.0, 40.0))]);
}

#[test]
fn several_unpositioned_roots_form_a_row() {
    let existing = [rect(0.0, 0.0, 1200.0, 800.0)];
    let plan = plan_root_placement(
        &existing,
        &[unpositioned(390.0, 844.0), unpositioned(390.0, 844.0)],
        false,
    );
    assert_eq!(plan, vec![Some((1280.0, 0.0)), Some((1750.0, 0.0))]);
}

#[test]
fn unpositioned_roots_on_an_empty_page_start_at_origin_then_row() {
    let plan = plan_root_placement(
        &[],
        &[unpositioned(390.0, 844.0), unpositioned(390.0, 844.0)],
        false,
    );
    assert_eq!(plan, vec![None, Some((470.0, 0.0))]);
}

#[test]
fn positioned_group_moves_together_and_keeps_its_arrangement() {
    // A screen and a note under it; the screen overlaps existing content.
    let existing = [rect(0.0, 0.0, 1200.0, 800.0)];
    let plan = plan_root_placement(
        &existing,
        &[
            positioned(100.0, 100.0, 390.0, 844.0),
            positioned(100.0, 1000.0, 390.0, 100.0),
        ],
        false,
    );
    assert_eq!(plan, vec![Some((1280.0, 0.0)), Some((1280.0, 900.0))]);
}

#[test]
fn unpositioned_root_lands_right_of_positioned_ones_from_the_same_insert() {
    let plan = plan_root_placement(
        &[],
        &[
            positioned(0.0, 0.0, 390.0, 844.0),
            unpositioned(390.0, 844.0),
        ],
        false,
    );
    assert_eq!(plan, vec![None, Some((470.0, 0.0))]);
}

#[test]
fn placement_aligns_to_the_top_most_existing_root() {
    let existing = [
        rect(0.0, 300.0, 200.0, 200.0),
        rect(400.0, -50.0, 100.0, 100.0),
    ];
    let plan = plan_root_placement(&existing, &[unpositioned(100.0, 100.0)], false);
    assert_eq!(plan, vec![Some((580.0, -50.0))]);
}
