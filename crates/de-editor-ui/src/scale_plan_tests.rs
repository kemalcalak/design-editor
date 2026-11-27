use super::*;
use crate::layout_scene::{NodeKind, SceneNode, ScenePage};
use crate::Rect;
use serde_json::json;

/// A frame at (100, 50) holding a free child at (20, 10).
fn fixture() -> (EditorState, LayoutScene) {
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(json!({
        "version": "1.0",
        "children": [
            {"type": "frame", "id": "f", "x": 100, "y": 50, "width": 200, "height": 100,
             "children": [{"type": "rectangle", "id": "c", "x": 20, "y": 10, "width": 40, "height": 40}]},
            {"type": "rectangle", "id": "r", "x": 400, "y": 50, "width": 100, "height": 50}
        ]
    }))
    .expect("doc");
    let state = EditorState::from_document(doc);
    let mut child = SceneNode::leaf("c", NodeKind::Rect);
    child.bounds = Rect::xywh(120.0, 60.0, 40.0, 40.0);
    let mut frame = SceneNode::leaf("f", NodeKind::Frame);
    frame.bounds = Rect::xywh(100.0, 50.0, 200.0, 100.0);
    frame.children = vec![child];
    let mut rect = SceneNode::leaf("r", NodeKind::Rect);
    rect.bounds = Rect::xywh(400.0, 50.0, 100.0, 50.0);
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p1".into(),
            name: "Page 1".into(),
            children: vec![frame, rect],
            image_thumbs: Default::default(),
        }],
        active_page_index: 0,
    };
    (state, scene)
}

#[test]
fn the_plan_unions_resolved_bounds() {
    let (state, scene) = fixture();
    let plan = ScalePlan::new(&state, &scene, &[NodeId::new("f"), NodeId::new("r")]).expect("plan");
    assert_eq!(
        plan.bounds,
        DocRect {
            x: 100.0,
            y: 50.0,
            w: 400.0,
            h: 100.0
        }
    );
    let scaled = plan.scaled_bounds(0.5, ScaleAnchor::Center);
    assert_eq!((scaled.x, scaled.w), (200.0, 200.0));
}

#[test]
fn a_nested_target_gets_its_pivot_in_parent_space() {
    let (mut state, scene) = fixture();
    let plan = ScalePlan::new(&state, &scene, &[NodeId::new("c")]).expect("plan");
    // Bottom-right of the child is page (160, 100) = frame-space (60, 50).
    let request = plan.request(2.0, ScaleAnchor::BottomRight, VariableMode::KeepBound);
    assert_eq!(request.targets[0].pivot, Some((60.0, 50.0)));
    state.scale_nodes(&request).expect("scaled");
    let c = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("c"))
        .expect("child");
    assert_eq!((c.base().x, c.base().y), (Some(-20.0), Some(-30.0)));
}

#[test]
fn handles_pull_from_the_opposite_point() {
    let b = DocRect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 50.0,
    };
    assert_eq!(
        opposite_anchor(SelectionHandle::TopLeft),
        ScaleAnchor::BottomRight
    );
    assert_eq!(opposite_anchor(SelectionHandle::Right), ScaleAnchor::Left);
    assert_eq!(drag_factor(SelectionHandle::Right, b, 50.0, 0.0), 1.5);
    assert_eq!(drag_factor(SelectionHandle::Left, b, 50.0, 0.0), 0.5);
    assert_eq!(drag_factor(SelectionHandle::Bottom, b, 0.0, 25.0), 1.5);
    // A corner follows the axis that moved most (relative to its size).
    assert_eq!(
        drag_factor(SelectionHandle::BottomRight, b, 10.0, 25.0),
        1.5
    );
    assert_eq!(drag_factor(SelectionHandle::Right, b, -500.0, 0.0), 0.01);
}

#[test]
fn a_drag_rescales_from_the_original_and_lands_as_one_undo_step() {
    let (mut state, scene) = fixture();
    state.set_single_selection(NodeId::new("r"));
    let mut drag =
        ScaleDrag::begin(&state, &scene, SelectionHandle::Right, (0.0, 0.0)).expect("drag");
    assert!(drag.update(&mut state, 100.0, 0.0));
    assert!(drag.update(&mut state, 50.0, 0.0));
    let width = |s: &EditorState| {
        de_editor_core::walkers::find_node(s.active_children(), &NodeId::new("r"))
            .and_then(|n| n.width_px())
    };
    assert_eq!(width(&state), Some(150.0), "not compounded");
    let depth = state.history.past.len();
    assert!(drag.finish(&mut state, 0));
    assert_eq!(state.history.past.len(), depth + 1);
    assert!(state.undo());
    assert_eq!(width(&state), Some(100.0));
}

#[test]
fn a_cancelled_drag_restores_the_selection() {
    let (mut state, scene) = fixture();
    state.set_single_selection(NodeId::new("r"));
    let mut drag =
        ScaleDrag::begin(&state, &scene, SelectionHandle::Bottom, (0.0, 0.0)).expect("drag");
    drag.update(&mut state, 0.0, 50.0);
    drag.cancel(&mut state);
    let r =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("r")).expect("r");
    assert_eq!(r.height_px(), Some(50.0));
}

#[test]
fn a_rotated_node_reads_the_pointer_along_its_own_axes() {
    let (mut state, mut scene) = fixture();
    // `r` turned a quarter clockwise: its width now runs down the page.
    scene.pages[0].children[1].rotation = std::f32::consts::FRAC_PI_2;
    state.set_single_selection(NodeId::new("r"));
    let mut drag =
        ScaleDrag::begin(&state, &scene, SelectionHandle::Right, (0.0, 0.0)).expect("drag");
    // Dragging the right handle 50 px down the page widens it by half.
    assert!(drag.update(&mut state, 0.0, 50.0));
    assert!((drag.factor - 1.5).abs() < 1e-6, "{}", drag.factor);
}
