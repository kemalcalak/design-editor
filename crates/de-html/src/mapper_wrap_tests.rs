use crate::{import_html, HtmlImportOptions};
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::{FrameNode, NumberOrExpression, PenNode};

fn container(html: &str) -> (FrameNode, Vec<String>) {
    let result = import_html(html, &HtmlImportOptions::default());
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!("expected a root frame")
    };
    let PenNode::Frame(flex) = &root.children.as_ref().unwrap()[0] else {
        panic!("expected the flex container")
    };
    (flex.clone(), result.warnings)
}

#[test]
fn a_wrapping_row_becomes_native_wrap_with_its_children_in_place() {
    let card = "<div style='width:300px;height:10px'>x</div>";
    let (flex, warnings) = container(&format!(
        "<div style='display:flex;flex-wrap:wrap;width:700px;column-gap:24px;row-gap:8px'>\
         {card}{card}{card}</div>"
    ));
    assert_eq!(flex.container.layout, Some(LayoutMode::Horizontal));
    assert_eq!(flex.container.wrap, Some(true));
    assert_eq!(flex.container.gap, Some(NumberOrExpression::Number(24.0)));
    assert_eq!(
        flex.container.row_gap,
        Some(NumberOrExpression::Number(8.0))
    );
    // No synthetic "Wrap row" frames: the three cards stay direct children.
    assert_eq!(flex.children.as_ref().map(Vec::len), Some(3));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn an_equal_gap_stays_a_single_gap() {
    let (flex, _) = container(
        "<div style='display:flex;flex-wrap:wrap;gap:12px;width:200px'><span>a</span><span>b</span></div>",
    );
    assert_eq!(flex.container.gap, Some(NumberOrExpression::Number(12.0)));
    assert_eq!(flex.container.row_gap, None);
}

#[test]
fn column_wrap_and_dropped_align_content_still_warn() {
    let (flex, warnings) = container(
        "<div style='display:flex;flex-direction:column;flex-wrap:wrap;height:100px'><div>a</div></div>",
    );
    assert_eq!(flex.container.wrap, None);
    assert!(
        warnings.iter().any(|w| w.contains("column")),
        "{warnings:?}"
    );
    let (_, warnings) = container(
        "<div style='display:flex;flex-wrap:wrap;align-content:space-between;width:300px'><div>a</div></div>",
    );
    assert!(
        warnings.iter().any(|w| w.contains("align-content")),
        "{warnings:?}"
    );
}
