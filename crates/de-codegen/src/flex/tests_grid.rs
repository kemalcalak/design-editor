//! Grid and wrap auto layout in the HTML targets (#379).

use super::tests::{assert_golden, fixture};
use super::*;

#[test]
fn grid_matches_the_golden_html() {
    assert_golden("grid.html", &Html.generate(&fixture("grid")));
}

#[test]
fn grid_matches_the_golden_html_tailwind() {
    assert_golden(
        "grid.tailwind.html",
        &HtmlTailwind.generate(&fixture("grid")),
    );
}

#[test]
fn grid_css_keeps_tracks_cells_and_wrap() {
    let html = Html.generate(&fixture("grid"));
    for needle in [
        "display: grid",
        "grid-template-columns: repeat(3, 1fr)",
        "row-gap: 16px",
        "column-gap: 24px",
        "grid-column: 1 / span 2",
        "justify-self: end",
        "grid-template-columns: 200px minmax(120px, 1fr) auto",
        "grid-template-rows: 1fr 40px",
        "flex-wrap: wrap",
    ] {
        assert!(html.contains(needle), "missing {needle}\n{html}");
    }
}

#[test]
fn grid_tailwind_uses_grid_utilities() {
    let html = HtmlTailwind.generate(&fixture("grid"));
    for needle in [
        "grid-cols-3",
        "gap-y-[16px]",
        "gap-x-[24px]",
        "col-[1/span_2]",
        "flex-wrap",
        "col-start-2",
        "row-start-2",
    ] {
        assert!(html.contains(needle), "missing {needle}\n{html}");
    }
}
