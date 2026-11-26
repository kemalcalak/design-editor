//! #379 over MCP: a 3-column card grid and a wrapping tag list are one
//! frame each through `batch_design`, `U()` switches a frame to grid, and
//! `snapshot_layout` reports the tracks and every child's cell.

use super::test_fixtures::sample;
use super::{McpTool, ToolOutcome};
use crate::batch_design_snapshot;
use crate::read_tools::snapshot_layout_snapshot;
use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::{GridTemplate, LayoutMode, PenNode};
use serde_json::Value;
use std::collections::BTreeMap;

fn run(state: &mut EditorState, operations: &str) -> Value {
    let tool = batch_design_snapshot(state);
    let args = BTreeMap::from([("operations".to_string(), operations.to_string())]);
    let (json, cmd) = match tool.call(&args) {
        ToolOutcome::OkJsonWithCommand(json, cmd) => (json, cmd),
        other => panic!("expected a command, got {other:?}"),
    };
    assert!(state.apply(cmd));
    serde_json::from_str(&json).unwrap()
}

fn find<'a>(nodes: &'a [PenNode], name: &str) -> Option<&'a PenNode> {
    nodes.iter().find_map(|n| {
        (n.base().name.as_deref() == Some(name))
            .then_some(n)
            .or_else(|| n.children().and_then(|c| find(c, name)))
    })
}

fn container(node: &PenNode) -> &jian_ops_schema::node::ContainerProps {
    match node {
        PenNode::Frame(f) => &f.container,
        other => panic!("{other:?}"),
    }
}

const CARD_GRID: &str = r##"grid=I(null, {"type":"frame","name":"Cards","x":0,"y":0,"width":340,"height":"fit_content","layout":"grid","columns":3,"gap":20,"padding":10})
I(grid, {"type":"frame","name":"Card 1","width":"fill_container","height":60,"gridColumn":"span 2"})
I(grid, {"type":"frame","name":"Card 2","width":"fill_container","height":60})
I(grid, {"type":"frame","name":"Card 3","width":"fill_container","height":60})
I(grid, {"type":"frame","name":"Card 4","width":"fill_container","height":60})
tags=I(null, {"type":"frame","name":"Tags","x":0,"y":400,"width":150,"height":"fit_content","layout":"horizontal","wrap":true,"gap":10,"rowGap":4})
I(tags, {"type":"frame","name":"Tag 1","width":60,"height":20})
I(tags, {"type":"frame","name":"Tag 2","width":60,"height":20})
I(tags, {"type":"frame","name":"Tag 3","width":60,"height":20})"##;

#[test]
fn card_grid_and_tag_list_are_one_frame_each() {
    let mut state = sample();
    run(&mut state, CARD_GRID);
    let cards = find(state.active_children(), "Cards").expect("grid");
    let c = container(cards);
    assert_eq!(c.layout, Some(LayoutMode::Grid));
    assert_eq!(c.columns, Some(GridTemplate::Count(3)));
    assert_eq!(cards.children().map(Vec::len), Some(4));
    let first = find(state.active_children(), "Card 1").unwrap();
    assert_eq!(first.base().grid_column.map(|p| p.span), Some(2));
    let tags = find(state.active_children(), "Tags").expect("tags");
    assert_eq!(container(tags).wrap, Some(true));
}

#[test]
fn update_switches_a_frame_to_grid_with_figma_and_css_spellings() {
    let mut state = sample();
    run(
        &mut state,
        r##"row=I(null, {"type":"frame","name":"Row","width":300,"height":100,"layout":"horizontal"})
U(row, {"layout":{"type":"grid","columns":"repeat(2, 1fr)","rowGap":8}})"##,
    );
    let row = find(state.active_children(), "Row").unwrap();
    let c = container(row);
    assert_eq!(c.layout, Some(LayoutMode::Grid));
    assert_eq!(c.columns.as_ref().map(GridTemplate::len), Some(2));
    assert_eq!(
        c.row_gap,
        Some(jian_ops_schema::node::NumberOrExpression::Number(8.0))
    );

    run(
        &mut state,
        r##"I(null, {"type":"frame","name":"Chips","width":200,"height":40,"layoutMode":"HORIZONTAL","layoutWrap":"WRAP","counterAxisSpacing":6})"##,
    );
    let chips = find(state.active_children(), "Chips").unwrap();
    let c = container(chips);
    assert_eq!(c.wrap, Some(true));
    assert_eq!(
        c.row_gap,
        Some(jian_ops_schema::node::NumberOrExpression::Number(6.0))
    );
}

#[test]
fn snapshot_layout_reports_tracks_and_cells() {
    let mut state = sample();
    run(&mut state, CARD_GRID);
    let cards_id = find(state.active_children(), "Cards")
        .unwrap()
        .id_str()
        .to_string();
    let tool = snapshot_layout_snapshot(&state);
    let args = BTreeMap::from([
        ("parentId".to_string(), cards_id.clone()),
        ("maxDepth".to_string(), "1".to_string()),
    ]);
    let ToolOutcome::OkJson(json) = tool.call(&BTreeMap::new()) else {
        panic!("snapshot_layout failed")
    };
    let v: Value = serde_json::from_str(&json).unwrap();
    let entry = v["layout"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == cards_id.as_str())
        .expect("grid entry");
    let columns = entry["grid"]["columns"].as_array().expect("columns");
    assert_eq!(columns.len(), 3);
    let track = 280.0 / 3.0;
    let x0 = entry["x"].as_f64().unwrap() + 10.0;
    for (i, col) in columns.iter().enumerate() {
        let want = x0 + i as f64 * (track + 20.0);
        assert!((col["x"].as_f64().unwrap() - want).abs() < 1.0, "{col}");
        assert!(
            (col["width"].as_f64().unwrap() - track).abs() < 1.0,
            "{col}"
        );
    }
    assert_eq!(entry["grid"]["rows"].as_array().unwrap().len(), 2);

    let ToolOutcome::OkJson(json) = tool.call(&args) else {
        panic!("snapshot_layout(parentId) failed")
    };
    let v: Value = serde_json::from_str(&json).unwrap();
    let kids = v["layout"].as_array().unwrap();
    let cell = |i: usize| &kids[i]["cell"];
    assert_eq!(cell(0)["columnSpan"], 2);
    assert_eq!(
        (cell(0)["column"].as_u64(), cell(0)["row"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(cell(1)["column"], 3);
    assert_eq!(
        (cell(2)["column"].as_u64(), cell(2)["row"].as_u64()),
        (Some(1), Some(2))
    );
    assert_eq!(cell(3)["column"], 2);
}
