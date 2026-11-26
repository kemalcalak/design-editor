//! #379: an imported CSS grid / flex-wrap page lays out like the browser.
//!
//! The expected rects are what Chrome computes for the same markup (CSS
//! Grid §12 track sizing and the flex line-breaking algorithm on fixed-size
//! items, so the numbers are exact): equal `1fr` tracks share the content
//! width minus the gaps, a `span 2` item covers two tracks plus the gap
//! between them, and a wrapping row breaks before the item that no longer
//! fits its content box.

use de_editor_core::EditorState;
use jian_scene::layout_scene::SceneNode;

use crate::editor_state_to_layout_scene;

fn laid_out(html: &str) -> Vec<SceneNode> {
    let imported =
        de_html::import_html_document(html, &de_html::HtmlImportOptions::default(), None, None);
    let state = EditorState::from_document(imported.document);
    let scene = editor_state_to_layout_scene(&state);
    scene.active_page().expect("active page").children.clone()
}

fn dump(nodes: &[SceneNode], depth: usize, out: &mut String) {
    for n in nodes {
        out.push_str(&format!(
            "{}{} {:?} {} children\n",
            "  ".repeat(depth),
            n.id,
            n.bounds,
            n.children.len()
        ));
        dump(&n.children, depth + 1, out);
    }
}

fn tree(nodes: &[SceneNode]) -> String {
    let mut out = String::new();
    dump(nodes, 0, &mut out);
    out
}

fn container_with(nodes: &[SceneNode], count: usize, width: f32) -> Option<&SceneNode> {
    nodes.iter().find_map(|node| {
        (node.children.len() == count && (node.bounds.size.x - width).abs() < 0.5)
            .then_some(node)
            .or_else(|| container_with(&node.children, count, width))
    })
}

/// `[x, y, w, h]` of each child, relative to the container's content origin.
fn rects(container: &SceneNode, inset_x: f32) -> Vec<[f32; 4]> {
    let o = container.bounds.origin;
    container
        .children
        .iter()
        .map(|c| {
            let b = c.bounds;
            [
                b.origin.x - o.x - inset_x,
                b.origin.y - o.y,
                b.size.x,
                b.size.y,
            ]
        })
        .collect()
}

fn assert_rects(actual: &[[f32; 4]], expected: &[[f32; 4]]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        for (x, y) in a.iter().zip(e) {
            assert!((x - y).abs() < 1.0, "got {actual:?}\nwant {expected:?}");
        }
    }
}

#[test]
fn css_grid_with_a_span_lays_out_like_the_browser() {
    let nodes = laid_out(
        r#"<style>
          .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; width: 960px; }
          .cell { height: 100px; background: #e2e8f0; }
          .wide { grid-column: span 2; }
        </style>
        <div class="grid">
          <div class="cell wide"></div><div class="cell"></div>
          <div class="cell"></div><div class="cell"></div>
        </div>"#,
    );
    let grid =
        container_with(&nodes, 4, 960.0).unwrap_or_else(|| panic!("the grid\n{}", tree(&nodes)));
    let track = (960.0 - 2.0 * 20.0) / 3.0;
    assert_rects(
        &rects(grid, 0.0),
        &[
            [0.0, 0.0, 2.0 * track + 20.0, 100.0],
            [2.0 * track + 40.0, 0.0, track, 100.0],
            [0.0, 120.0, track, 100.0],
            [track + 20.0, 120.0, track, 100.0],
        ],
    );
    assert!(
        (grid.bounds.size.y - 220.0).abs() < 0.5,
        "{:?}",
        grid.bounds
    );
}

#[test]
fn flex_wrap_cards_break_where_the_browser_breaks_them() {
    let nodes = laid_out(
        r#"<style>
          .cards { display: flex; flex-wrap: wrap; gap: 24px; width: 960px; }
          .card { width: 300px; height: 180px; background: #eef2ff; }
        </style>
        <div class="cards">
          <div class="card"></div><div class="card"></div>
          <div class="card"></div><div class="card"></div>
        </div>"#,
    );
    let cards =
        container_with(&nodes, 4, 960.0).unwrap_or_else(|| panic!("the cards\n{}", tree(&nodes)));
    assert_rects(
        &rects(cards, 0.0),
        &[
            [0.0, 0.0, 300.0, 180.0],
            [324.0, 0.0, 300.0, 180.0],
            [648.0, 0.0, 300.0, 180.0],
            [0.0, 204.0, 300.0, 180.0],
        ],
    );
}

#[test]
fn a_wrapping_row_breaks_inside_its_inline_padding() {
    let nodes = laid_out(
        r#"<div style="display:flex;flex-wrap:wrap;width:700px;box-sizing:border-box;padding:0 100px">
          <div style="width:230px;height:20px;background:#000"></div>
          <div style="width:230px;height:20px;background:#000"></div>
          <div style="width:230px;height:20px;background:#000"></div>
        </div>"#,
    );
    let row =
        container_with(&nodes, 3, 700.0).unwrap_or_else(|| panic!("the row\n{}", tree(&nodes)));
    // Content box 500px: two 230px items fit, the third wraps.
    assert_rects(
        &rects(row, 100.0),
        &[
            [0.0, 0.0, 230.0, 20.0],
            [230.0, 0.0, 230.0, 20.0],
            [0.0, 20.0, 230.0, 20.0],
        ],
    );
}
