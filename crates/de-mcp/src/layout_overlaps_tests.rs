//! Tests for `layout_overlaps` (#265).

use std::collections::BTreeMap;

use jian_scene::layout_scene::{NodeKind, SceneNode};

use super::layout_overlaps::{new_overlap_warnings, overlapping_pairs, sibling_overlaps};
use super::McpTool;

fn boxed(id: &str, kind: NodeKind, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, kind);
    n.bounds.origin.x = x;
    n.bounds.origin.y = y;
    n.bounds.size.x = w;
    n.bounds.size.y = h;
    n
}

fn text(id: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = boxed(id, NodeKind::Text, x, y, w, h);
    n.text = Some("Words".into());
    n
}

#[test]
fn wrapped_headline_over_the_next_line_is_an_overlap() {
    // The issue's case: a 2-line 56 px headline at y 238 runs over the
    // line placed at y 302.
    let siblings = vec![
        text("headline", 0.0, 238.0, 760.0, 134.0),
        text("subline", 0.0, 302.0, 600.0, 40.0),
    ];
    let found = sibling_overlaps(&siblings);
    assert_eq!(found.get("headline"), Some(&vec!["subline".to_string()]));
    assert_eq!(found.get("subline"), Some(&vec!["headline".to_string()]));
}

#[test]
fn backgrounds_touching_edges_and_watermarks_are_not_overlaps() {
    let mut watermark = text("numeral", 0.0, 0.0, 400.0, 400.0);
    watermark.opacity = 0.06;
    let siblings = vec![
        boxed("bg", NodeKind::Rect, 0.0, 0.0, 800.0, 600.0),
        text("title", 40.0, 40.0, 300.0, 60.0),
        text("next", 40.0, 102.0, 300.0, 40.0),
        watermark,
        boxed("photo", NodeKind::Rect, 500.0, 300.0, 100.0, 100.0),
    ];
    assert!(sibling_overlaps(&siblings).is_empty());
}

#[test]
fn page_roots_are_skipped_and_pairs_are_ordered() {
    let mut root = boxed("root", NodeKind::Frame, 0.0, 0.0, 800.0, 600.0);
    root.children = vec![
        text("z-line", 0.0, 0.0, 300.0, 100.0),
        text("a-line", 0.0, 50.0, 300.0, 100.0),
    ];
    let other_root = text("other", 0.0, 0.0, 300.0, 100.0);
    assert_eq!(
        overlapping_pairs(&[root, other_root]),
        vec![("a-line".to_string(), "z-line".to_string())]
    );
}

#[test]
fn batch_design_warns_about_the_overlap_its_write_creates() {
    let state = super::test_fixtures::sample();
    let tool = super::batch_design_snapshot(&state).with_overlap_warnings();
    let program = r##"f=I(document, {"type":"frame","name":"Hero","x":4000,"y":0,"width":900,"height":600,"layout":"none"})
h=I(f, {"type":"text","name":"Headline","x":40,"y":238,"width":760,"fontSize":56,"textGrowth":"fixed-width","content":"Yapay zekayla tasarım yapmanın en hızlı yolu burada"})
s=I(f, {"type":"text","name":"Subline","x":40,"y":302,"width":600,"fontSize":20,"content":"Second line"})"##;
    let mut args = BTreeMap::new();
    args.insert("operations".to_string(), program.to_string());
    let json = match tool.call(&args) {
        super::ToolOutcome::OkJsonWithCommand(json, _) => json,
        other => panic!("expected a write, got {other:?}"),
    };
    let result: serde_json::Value = serde_json::from_str(&json).expect("json");
    let warnings = result["warnings"].as_array().expect("warnings");
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().is_some_and(|w| w.contains(" overlaps "))),
        "{result}"
    );

    args.insert("allowOverlap".to_string(), "true".to_string());
    let json = match tool.call(&args) {
        super::ToolOutcome::OkJsonWithCommand(json, _) => json,
        other => panic!("expected a write, got {other:?}"),
    };
    assert!(!json.contains(" overlaps "), "{json}");
}

#[test]
fn an_overlap_that_was_already_there_is_not_repeated() {
    let mut state = super::test_fixtures::sample();
    let tool = super::batch_design_snapshot(&state).with_overlap_warnings();
    let mut args = BTreeMap::new();
    args.insert(
        "operations".to_string(),
        r##"f=I(document, {"type":"frame","name":"Card","x":4000,"y":0,"width":400,"height":300,"layout":"none"})
a=I(f, {"type":"text","name":"A","x":0,"y":0,"width":200,"height":60,"textGrowth":"fixed-width-height","content":"First"})
b=I(f, {"type":"text","name":"B","x":0,"y":20,"width":200,"height":60,"textGrowth":"fixed-width-height","content":"Second"})"##
            .to_string(),
    );
    let cmd = match tool.call(&args) {
        super::ToolOutcome::OkJsonWithCommand(_, cmd) => cmd,
        other => panic!("expected a write, got {other:?}"),
    };
    assert!(state.apply(cmd));
    use de_editor_core::PenNodeExt;
    let card = state
        .active_children()
        .iter()
        .find(|n| n.base().name.as_deref() == Some("Card"))
        .expect("card")
        .id_str()
        .to_string();
    let touch = de_editor_core::EditorCommand::SetNodeRotation {
        node_id: de_editor_core::NodeId::new(&card),
        degrees: 0.0,
    };
    assert!(new_overlap_warnings(&state, &touch).is_empty());
}
