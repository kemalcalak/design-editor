//! Interaction routing against a browser node at document (100, 100),
//! 400 × 300, its page laid out at 400 × 300 CSS px. The viewport pans
//! by (300, 200), so the node sits at screen (400, 300) – (800, 600) at
//! zoom 1.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::{EditorState, NodeId};

use super::*;
use crate::layout_scene::{LayoutScene, NodeKind, SceneNode, ScenePage};
use crate::Rect;

const VW: f32 = 2000.0;
const VH: f32 = 1200.0;

fn rig() -> (EditorState, LayoutScene) {
    let doc = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{ "type": "browser", "id": "b1", "x": 100, "y": 100,
                       "width": 400, "height": 300, "url": "https://example.com" }],
    }))
    .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state.editor_ui.sidebar_open = false;
    state.viewport.pan_x = 300.0;
    state.viewport.pan_y = 200.0;
    state.viewport.zoom = 1.0;
    let mut node = SceneNode::leaf("b1", NodeKind::Other("browser".into()));
    node.bounds = Rect::xywh(100.0, 100.0, 400.0, 300.0);
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "p".into(),
            name: "Page".into(),
            children: vec![node],
        }],
        active_page_index: 0,
    };
    (state, scene)
}

fn interactive() -> (EditorState, LayoutScene) {
    let (mut state, scene) = rig();
    assert!(state.enter_browser_interaction(NodeId::new("b1")));
    (state, scene)
}

fn none() -> InputModifiers {
    InputModifiers::default()
}

fn press(
    flow: &mut BrowserInteraction,
    state: &mut EditorState,
    scene: &LayoutScene,
    pressed: bool,
    x: f32,
    y: f32,
    now_ms: u64,
) -> ButtonOutcome {
    flow.pointer_button(
        state,
        scene,
        pressed,
        MouseButton::Left,
        x,
        y,
        VW,
        VH,
        none(),
        now_ms,
    )
}

#[test]
fn nothing_is_routed_without_an_interactive_node() {
    let (mut state, scene) = rig();
    let mut flow = BrowserInteraction::new();
    assert!(!flow.pointer_move(&state, &scene, 500.0, 400.0, VW, VH, none()));
    assert_eq!(
        press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0),
        ButtonOutcome::Editor
    );
    assert!(!flow.wheel(&state, &scene, 500.0, 400.0, 0.0, -10.0, VW, VH, none()));
    let key = KeyInput {
        key: "a".into(),
        code: "KeyA".into(),
        text: Some("a".into()),
        modifiers: none(),
    };
    assert_eq!(flow.key(&mut state, key, true), KeyOutcome::Editor);
    assert!(flow.take().is_none());
}

#[test]
fn a_click_on_the_page_maps_to_css_px() {
    let (mut state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    assert!(flow.over_page(&state, &scene, 500.0, 400.0, VW, VH));
    assert_eq!(
        press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0),
        ButtonOutcome::Page
    );
    assert!(flow.captures_pointer());
    assert_eq!(
        press(&mut flow, &mut state, &scene, false, 500.0, 400.0, 10),
        ButtonOutcome::Page
    );
    let (node, events) = flow.take().expect("queued");
    assert_eq!(node, "b1");
    assert_eq!(
        events,
        vec![
            BrowserInputEvent::MouseDown {
                x: 100.0,
                y: 100.0,
                button: MouseButton::Left,
                click_count: 1,
                buttons: 0,
                modifiers: none(),
            },
            BrowserInputEvent::MouseUp {
                x: 100.0,
                y: 100.0,
                button: MouseButton::Left,
                click_count: 1,
                buttons: 0,
                modifiers: none(),
            },
        ]
    );
    assert_eq!(state.interactive_browser(), Some(NodeId::new("b1")));
}

#[test]
fn zooming_the_canvas_keeps_the_page_point() {
    for zoom in [0.5_f32, 1.0, 2.0] {
        let (mut state, scene) = interactive();
        state.viewport.zoom = zoom;
        state.viewport.pan_x = 50.0;
        state.viewport.pan_y = 40.0;
        // The page point (40, 30) is document (140, 130).
        let (x, y) = (140.0 * zoom + 50.0, 130.0 * zoom + 40.0);
        let mut flow = BrowserInteraction::new();
        assert!(flow.pointer_move(&state, &scene, x, y, VW, VH, none()));
        let (_, events) = flow.take().unwrap();
        let BrowserInputEvent::MouseMove { x: px, y: py, .. } = events[0] else {
            panic!("a move");
        };
        assert!(
            (px - 40.0).abs() < 1e-3 && (py - 30.0).abs() < 1e-3,
            "zoom {zoom}"
        );
    }
}

#[test]
fn a_second_press_in_time_is_a_double_click_for_the_page() {
    let (mut state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0);
    press(&mut flow, &mut state, &scene, false, 500.0, 400.0, 50);
    press(&mut flow, &mut state, &scene, true, 501.0, 401.0, 200);
    press(&mut flow, &mut state, &scene, false, 501.0, 401.0, 250);
    press(&mut flow, &mut state, &scene, true, 501.0, 401.0, 2000);
    let counts: Vec<u32> = flow
        .take()
        .unwrap()
        .1
        .iter()
        .filter_map(|e| match e {
            BrowserInputEvent::MouseDown { click_count, .. } => Some(*click_count),
            _ => None,
        })
        .collect();
    assert_eq!(counts, [1, 2, 1]);
}

#[test]
fn a_drag_started_on_the_page_keeps_the_pointer_outside_the_box() {
    let (mut state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0);
    assert!(flow.pointer_move(&state, &scene, 900.0, 400.0, VW, VH, none()));
    assert_eq!(
        press(&mut flow, &mut state, &scene, false, 900.0, 400.0, 30),
        ButtonOutcome::Page
    );
    assert!(!flow.captures_pointer());
    assert!(!flow.pointer_move(&state, &scene, 900.0, 400.0, VW, VH, none()));
    let (_, events) = flow.take().unwrap();
    assert!(matches!(
        events[1],
        BrowserInputEvent::MouseMove { x, buttons: 1, .. } if x == 500.0
    ));
    assert_eq!(state.interactive_browser(), Some(NodeId::new("b1")));
}

#[test]
fn a_press_outside_the_node_leaves_the_mode() {
    let (mut state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    flow.pointer_move(&state, &scene, 500.0, 400.0, VW, VH, none());
    assert_eq!(
        press(&mut flow, &mut state, &scene, true, 1000.0, 900.0, 0),
        ButtonOutcome::Left
    );
    assert_eq!(state.interactive_browser(), None);
    let (_, events) = flow.take().expect("the move queued before still goes out");
    assert_eq!(events.len(), 1, "nothing was held, so nothing is released");
    // A press on the chrome over the node (the properties card on the
    // right edge) ends it too, so typing then goes to the panel.
    assert!(state.enter_browser_interaction(NodeId::new("b1")));
    // The node spans screen x 1700..2100, under the card.
    state.viewport.pan_x = VW - 400.0;
    let (x, y) = (VW - 40.0, 400.0);
    assert!(!over_canvas(&state, x, y, VW, VH), "the card covers it");
    assert_eq!(
        press(&mut flow, &mut state, &scene, true, x, y, 0),
        ButtonOutcome::Left
    );
    assert_eq!(state.interactive_browser(), None);
}

#[test]
fn moves_coalesce_and_wheels_scroll_the_page() {
    let (state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    for x in [450.0, 460.0, 470.0] {
        flow.pointer_move(&state, &scene, x, 400.0, VW, VH, none());
    }
    // A two-finger swipe up (content moves up 30 px) scrolls the page down.
    assert!(flow.wheel(&state, &scene, 470.0, 400.0, 0.0, -10.0, VW, VH, none()));
    assert!(flow.wheel(&state, &scene, 470.0, 400.0, 0.0, -20.0, VW, VH, none()));
    assert!(!flow.wheel(&state, &scene, 1000.0, 900.0, 0.0, -20.0, VW, VH, none()));
    let (_, events) = flow.take().unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], BrowserInputEvent::MouseMove { x, .. } if x == 70.0));
    assert!(matches!(
        events[1],
        BrowserInputEvent::Wheel { delta_y, .. } if delta_y == 30.0
    ));
}

#[test]
fn escape_leaves_and_other_keys_reach_the_page() {
    let (mut state, scene) = interactive();
    let _ = scene;
    let mut flow = BrowserInteraction::new();
    let key = |k: &str, code: &str, text: Option<&str>| KeyInput {
        key: k.into(),
        code: code.into(),
        text: text.map(str::to_string),
        modifiers: none(),
    };
    assert_eq!(
        flow.key(&mut state, key("a", "KeyA", Some("a")), true),
        KeyOutcome::Page
    );
    assert_eq!(
        flow.key(&mut state, key("a", "KeyA", None), false),
        KeyOutcome::Page
    );
    assert!(flow.insert_text(&state, "ü"));
    let (_, events) = flow.take().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(
        flow.key(&mut state, key("Escape", "Escape", None), true),
        KeyOutcome::Left
    );
    assert_eq!(state.interactive_browser(), None);
    assert!(flow.take().is_none());
    assert_eq!(
        flow.key(&mut state, key("a", "KeyA", Some("a")), true),
        KeyOutcome::Editor
    );
}

/// The canvas marks the interactive node with an accent ring and hides
/// its resize handles.
#[test]
fn interaction_mode_paints_a_ring_instead_of_handles() {
    use crate::widgets::canvas_viewport::CanvasViewport;
    use crate::widgets::test_capture_backend::CaptureBackend;
    use crate::widgets::{PaintCx, Widget};

    let paint = |state: &EditorState, scene: &LayoutScene| {
        let mut backend = CaptureBackend::default();
        let view = CanvasViewport::from_editor(state, scene);
        view.paint(
            &mut PaintCx {
                backend: &mut backend,
            },
            Rect::xywh(0.0, 0.0, VW, VH),
        );
        backend
    };
    let ring_width = 3.0;
    let (mut state, scene) = rig();
    state.set_single_selection(NodeId::new("b1"));
    let selected = paint(&state, &scene);
    assert!(!selected.round_strokes.iter().any(|s| s.3 == ring_width));
    let handles = selected.round_strokes.len();
    assert!(handles >= 4, "corner handles of the selection");

    assert!(state.enter_browser_interaction(NodeId::new("b1")));
    let interacting = paint(&state, &scene);
    let rings: Vec<_> = interacting
        .round_strokes
        .iter()
        .filter(|s| s.3 == ring_width)
        .collect();
    assert_eq!(rings.len(), 1, "one accent ring");
    // Just outside the node's box (screen 400..800 × 300..600).
    let ring = rings[0].0;
    assert!(ring.origin.x < 400.0 && ring.origin.x > 394.0);
    assert!(ring.size.x > 400.0 && ring.size.x < 412.0);
    assert_eq!(interacting.round_strokes.len(), 1, "no handles");
}

#[test]
fn ending_the_mode_mid_drag_releases_the_button_on_the_page() {
    let (mut state, scene) = interactive();
    let mut flow = BrowserInteraction::new();
    press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0);
    flow.pointer_move(&state, &scene, 520.0, 410.0, VW, VH, none());
    let _ = flow.take();
    // Another node gets selected (an agent, the layer panel) mid-drag.
    state.set_single_selection(NodeId::NONE);
    assert!(flow.sync(&mut state));
    assert!(!flow.captures_pointer());
    let (node, events) = flow.take().expect("a release");
    assert_eq!(node, "b1");
    assert!(matches!(
        events[..],
        [BrowserInputEvent::MouseUp { x, y, button: MouseButton::Left, buttons: 0, .. }]
            if x == 120.0 && y == 110.0
    ));
    // Losing focus mid-press does the same without ending the mode.
    let (mut state, scene) = interactive();
    press(&mut flow, &mut state, &scene, true, 500.0, 400.0, 0);
    let _ = flow.take();
    flow.release_pointer();
    assert!(!flow.captures_pointer());
    assert_eq!(flow.take().unwrap().1.len(), 1);
    assert!(state.interactive_browser().is_some());
}

#[test]
fn a_double_click_on_a_browser_enters_the_mode() {
    let (mut state, _) = rig();
    let mut flow = BrowserInteraction::new();
    assert!(!flow.enter_at(&mut state, &[]));
    assert!(flow.enter_at(&mut state, &[NodeId::new("b1")]));
    assert_eq!(state.interactive_browser(), Some(NodeId::new("b1")));
}
