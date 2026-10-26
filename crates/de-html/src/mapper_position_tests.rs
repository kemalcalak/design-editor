use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;

use crate::{import_html, HtmlImportOptions};

fn children(frame: &FrameNode) -> &[PenNode] {
    frame.children.as_deref().unwrap_or_default()
}

fn nested_position(html: &str) -> (Option<f64>, Option<f64>) {
    let result = import_html(html, &HtmlImportOptions::default());
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let PenNode::Frame(positioned) = &children(root)[0] else {
        panic!()
    };
    let PenNode::Frame(static_child) = &children(positioned)[0] else {
        panic!()
    };
    let PenNode::Frame(target) = &children(static_child)[0] else {
        panic!()
    };
    (target.base.x, target.base.y)
}

#[test]
fn absolute_offsets_skip_static_ancestors() {
    let position = nested_position(
        "<div style='position:relative;width:300px;height:200px'>\
           <div style='width:100px;height:80px'>\
             <span style='position:absolute;right:0;bottom:0;width:50px;height:20px'></span>\
           </div>\
         </div>",
    );
    assert_eq!(position, (Some(250.0), Some(180.0)));
}

#[test]
fn fixed_offsets_use_the_viewport() {
    let position = nested_position(
        "<div style='position:relative;width:300px;height:200px'>\
           <div style='width:100px;height:80px'>\
             <span style='position:fixed;right:0;bottom:0;width:50px;height:20px'></span>\
           </div>\
         </div>",
    );
    assert_eq!(position, (Some(1390.0), Some(880.0)));
}

#[test]
fn absolute_viewport_unit_offsets_use_the_viewport() {
    let position = nested_position(
        "<div style='position:relative;width:300px;height:200px'>\
           <div style='width:100px;height:80px'>\
             <span style='position:absolute;left:10vw;top:10vh'></span>\
           </div>\
         </div>",
    );
    assert_eq!(position, (Some(144.0), Some(90.0)));
}

#[test]
fn right_and_bottom_offsets_may_place_a_larger_child_at_negative_coordinates() {
    let position = nested_position(
        "<div style='position:relative;width:10px;height:10px'>\
           <div><span style='position:absolute;right:0;bottom:0;width:20px;height:30px'></span></div>\
         </div>",
    );
    assert_eq!(position, (Some(-10.0), Some(-20.0)));
}

#[test]
fn relative_offsets_and_sticky_position_report_static_canvas_degradation() {
    let result = import_html(
        "<div style='display:flex'>\
           <span style='position:relative;left:5px'>first</span><span>second</span>\
           <span style='position:sticky;top:0'>sticky</span>\
         </div>",
        &HtmlImportOptions::default(),
    );
    // The span is inline-level, so it has no definite box to reserve in flow
    // and the wrapper cannot be built. The warning now says exactly that
    // instead of describing a wrapper that was never created.
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("CSS in-flow offset dropped")),
        "{:?}",
        result.warnings
    );
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("position:sticky")));
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let PenNode::Frame(row) = &children(root)[0] else {
        panic!()
    };
    assert_eq!(
        children(row).len(),
        3,
        "flow items must stay in source order"
    );
}

#[test]
fn absolute_full_size_image_bakes_the_positioned_containing_block() {
    let result = import_html(
        "<div style='position:relative;width:300px;height:200px'>\
           <img src='data:image/png;base64,AA==' \
                style='position:absolute;inset:0;width:100%;height:100%'>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let PenNode::Frame(positioned) = &children(root)[0] else {
        panic!()
    };
    let PenNode::Image(image) = &children(positioned)[0] else {
        panic!()
    };
    assert_eq!(image.width, Some(SizingBehavior::Number(300.0)));
    assert_eq!(image.height, Some(SizingBehavior::Number(200.0)));
    assert_eq!((image.base.x, image.base.y), (Some(0.0), Some(0.0)));
}

#[test]
fn opposing_absolute_insets_bake_the_remaining_outer_size() {
    let result = import_html(
        "<div style='position:relative;width:300px;height:200px'>\
           <span style='position:absolute;left:10px;right:20px;top:5px;bottom:15px'></span>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let PenNode::Frame(positioned) = &children(root)[0] else {
        panic!()
    };
    let PenNode::Frame(overlay) = &children(positioned)[0] else {
        panic!()
    };
    assert_eq!(overlay.container.width, Some(SizingBehavior::Number(270.0)));
    assert_eq!(
        overlay.container.height,
        Some(SizingBehavior::Number(180.0))
    );
    assert_eq!((overlay.base.x, overlay.base.y), (Some(10.0), Some(5.0)));
}

fn frame_child(frame: &FrameNode, index: usize) -> &FrameNode {
    match &children(frame)[index] {
        PenNode::Frame(child) => child,
        other => panic!("expected a frame, got {other:?}"),
    }
}

#[test]
fn absolute_insets_resolve_against_the_padding_box_of_a_padded_flex_child() {
    // A slide (`.sl`) pads its content; the positioned body (`.bd`) fills that
    // content box, so its absolute children stretch over 1136, not 1280.
    let result = import_html(
        "<div style='display:flex;flex-direction:column;width:1280px;height:720px;\
                     padding:60px 72px;box-sizing:border-box'>\
           <div style='flex:1;position:relative'>\
             <div style='position:absolute;left:0;right:0;top:0;height:40px'></div>\
           </div>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let slide = frame_child(root, 0);
    let body = frame_child(slide, 0);
    let row = frame_child(body, 0);
    assert_eq!(row.base.x, Some(0.0));
    assert_eq!(row.container.width, Some(SizingBehavior::Number(1136.0)));
}

#[test]
fn absolute_insets_keep_the_positioned_ancestors_own_padding() {
    // The padding box includes the ancestor's own padding: `right:0` lands on
    // its inner border edge, not on its content edge. Jian measures `x` / `y`
    // from the frame's outer corner (the CSS border folds into its padding),
    // so the left / top border shifts the origin too.
    let result = import_html(
        "<div style='position:relative;width:300px;height:200px;padding:20px;\
                     box-sizing:border-box;border:5px solid #000'>\
           <span style='position:absolute;right:0;bottom:0;width:50px;height:20px'></span>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let target = frame_child(frame_child(root, 0), 0);
    assert_eq!((target.base.x, target.base.y), (Some(245.0), Some(175.0)));
}

#[test]
fn root_flex_wrap_rows_fit_inside_the_body_margins() {
    // 2 × 716 = 1432 fits the 1440 viewport but not inside 8px body margins.
    let result = import_html(
        "<body style='display:flex;flex-wrap:wrap;margin:8px'>\
           <div style='width:716px;height:20px'>a</div>\
           <div style='width:716px;height:20px'>b</div>\
         </body>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let rows: Vec<_> = children(root)
        .iter()
        .map(|child| match child {
            PenNode::Frame(frame) => frame.base.name.clone().unwrap_or_default(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(rows, ["Wrap row", "Wrap row"]);
}

#[test]
fn a_bottom_pinned_row_in_a_layout_sized_body_is_anchored_to_its_bottom_edge() {
    // `.bd` grows with flex, so its height is unknown at import; the row
    // keeps its pin through an anchor that lays it out against the bottom.
    let result = import_html(
        "<div style='display:flex;flex-direction:column;width:1280px;height:720px;\
                     padding:60px 72px;box-sizing:border-box'>\
           <h2 style='margin:0;height:80px'>Title</h2>\
           <div style='flex:1;position:relative'>\
             <div style='position:absolute;left:0;right:0;bottom:0;height:200px'></div>\
           </div>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let body = frame_child(frame_child(root, 0), 1);
    let anchor = frame_child(body, 0);
    assert_eq!(anchor.base.name.as_deref(), Some("Anchor"));
    assert_eq!((anchor.base.x, anchor.base.y), (Some(0.0), Some(0.0)));
    assert_eq!(
        anchor.container.justify_content,
        Some(jian_ops_schema::node::container::JustifyContent::End)
    );
    let row = frame_child(anchor, 0);
    assert_eq!((row.base.x, row.base.y), (None, None));
    assert_eq!(row.container.width, Some(SizingBehavior::Number(1136.0)));
}

#[test]
fn an_inset_zero_box_in_a_layout_sized_body_fills_its_height() {
    // `.devs { position:absolute; inset:0 }` inside a `flex:1` body: its
    // height is the body's, which only layout knows, so it fills an anchor
    // instead of keeping a guess taken from the slide.
    let result = import_html(
        "<div style='display:flex;flex-direction:column;width:1280px;height:720px;\
                     padding:60px 72px;box-sizing:border-box'>\
           <h2 style='margin:0;height:80px'>Title</h2>\
           <div style='flex:1;position:relative'>\
             <div style='position:absolute;inset:0;display:flex;align-items:flex-end;\
                         padding-bottom:40px'><div style='width:100px;height:100px'></div></div>\
           </div>\
         </div>",
        &HtmlImportOptions::default(),
    );
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    let body = frame_child(frame_child(root, 0), 1);
    let anchor = frame_child(body, 0);
    assert_eq!(anchor.base.name.as_deref(), Some("Anchor"));
    let filled = frame_child(anchor, 0);
    assert_eq!(
        filled.container.height,
        Some(SizingBehavior::Keyword(
            jian_ops_schema::sizing::SizingKeyword::FillContainer
        ))
    );
    assert_eq!(filled.container.width, Some(SizingBehavior::Number(1136.0)));
}

fn anchored_child(html: &str) -> FrameNode {
    let result = import_html(html, &HtmlImportOptions::default());
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!()
    };
    frame_child(frame_child(root, 0), 0).clone()
}

#[test]
fn a_badge_hung_outside_a_corner_keeps_its_numeric_place() {
    // Padding cannot reach past the parent's edge, so a negative inset is
    // placed at map time as before.
    let badge = anchored_child(
        "<div style='position:relative;display:flex'>\
           <span style='position:absolute;right:-6px;top:-6px;background:#f00'>3</span>\
         </div>",
    );
    assert_ne!(badge.base.name.as_deref(), Some("Anchor"));
    assert_eq!(badge.base.y, Some(-6.0));
}

#[test]
fn a_transformed_pinned_child_keeps_its_numeric_place() {
    let tip = anchored_child(
        "<div style='position:relative'>\
           <div style='position:absolute;left:0;bottom:0;transform:translateY(100%)'>tip</div>\
         </div>",
    );
    assert_ne!(tip.base.name.as_deref(), Some("Anchor"));
}

#[test]
fn an_anchor_keeps_the_pinned_childs_end_margin() {
    let anchor = anchored_child(
        "<div style='position:relative'>\
           <div style='position:absolute;left:0;bottom:8px;margin-bottom:4px'>note</div>\
         </div>",
    );
    assert_eq!(anchor.base.name.as_deref(), Some("Anchor"));
    assert_eq!(
        anchor.container.padding,
        Some(jian_ops_schema::node::container::Padding::LtrB([
            0.0, 0.0, 12.0, 0.0
        ]))
    );
}
