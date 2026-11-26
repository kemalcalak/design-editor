use super::*;
use crate::interaction_edit::InteractionActionEdit;
use crate::NodeId;

/// A site page with a screen holding a button, an off-screen menu frame,
/// a component master and an instance of it, plus an "Overlays" page.
const DOC: &str = r#"{
  "version": "1.0",
  "children": [],
  "pages": [
    {"id": "p1", "name": "Site", "children": [
      {"type": "frame", "id": "home", "name": "Home", "screen": "/", "width": 400, "height": 800,
       "children": [{"type": "frame", "id": "btn", "name": "Button", "width": 80, "height": 40}]},
      {"type": "frame", "id": "menu", "name": "Menu", "x": 500, "width": 200, "height": 300},
      {"type": "frame", "id": "card", "name": "Card", "reusable": true, "x": 800,
       "width": 100, "height": 100},
      {"type": "ref", "id": "card-1", "ref": "card", "x": 1000}
    ]},
    {"id": "p2", "name": "Overlays", "children": [
      {"type": "frame", "id": "sheet", "name": "Sheet", "width": 400, "height": 300}
    ]}
  ]
}"#;

fn state() -> EditorState {
    let doc = serde_json::from_str(DOC).expect("fixture parses");
    let mut state = EditorState::from_document(doc);
    state.set_single_selection(NodeId::new("btn"));
    state
}

fn tap_overlay(state: &EditorState) -> Option<OverlayAction> {
    let node = state.selected_node()?;
    single_overlay(InteractionTrigger::Tap.actions(node.events()?))
}

#[test]
fn targets_list_frames_and_instances_on_every_page_but_the_own_board() {
    let state = state();
    let targets = overlay_targets(&state.doc, 0, "btn");
    let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["menu", "card-1", "sheet"]);
    assert_eq!(targets[2].page.as_deref(), Some("Overlays"));
    assert_eq!(targets[0].page, None);
}

#[test]
fn a_target_is_found_on_any_page() {
    let state = state();
    assert_eq!(
        find_overlay_target(&state.doc, "sheet").map(|(page, _)| page),
        Some(1)
    );
    assert!(find_overlay_target(&state.doc, "nope").is_none());
}

#[test]
fn picking_another_target_keeps_the_options() {
    let mut state = state();
    let open = |target: &str| InteractionActionEdit::OpenOverlay {
        target: target.into(),
    };
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &open("menu")));
    assert!(state.set_selected_overlay_option(
        InteractionTrigger::Tap,
        &OverlayOptionEdit::Position(OverlayPosition::BottomCenter)
    ));
    assert!(state.set_selected_overlay_option(
        InteractionTrigger::Tap,
        &OverlayOptionEdit::Background(true)
    ));
    assert!(state.set_selected_interaction_action(InteractionTrigger::Tap, &open("sheet")));
    let Some(OverlayAction::Open(spec)) = tap_overlay(&state) else {
        panic!("an open overlay");
    };
    assert_eq!(spec.target, "sheet");
    assert_eq!(spec.position, OverlayPosition::BottomCenter);
    assert_eq!(spec.background.as_deref(), Some(DEFAULT_OVERLAY_DIM));
}

#[test]
fn options_edit_the_single_overlay_and_refuse_no_ops() {
    let mut state = state();
    assert!(state.set_selected_interaction_action(
        InteractionTrigger::Tap,
        &InteractionActionEdit::OpenOverlay {
            target: "menu".into()
        }
    ));
    let tap = InteractionTrigger::Tap;
    assert!(!state.set_selected_overlay_option(tap, &OverlayOptionEdit::CloseOnOutsideClick(true)));
    assert!(state.set_selected_overlay_option(tap, &OverlayOptionEdit::CloseOnOutsideClick(false)));
    assert!(state.set_selected_overlay_option(
        tap,
        &OverlayOptionEdit::Animation(OverlayAnimationKind::MoveIn)
    ));
    assert!(state.set_selected_overlay_option(tap, &OverlayOptionEdit::Direction(OverlayEdge::Top)));
    assert!(state
        .set_selected_overlay_option(tap, &OverlayOptionEdit::BackgroundHex("#ff000080".into())));
    assert!(!state.set_selected_overlay_option(
        tap,
        &OverlayOptionEdit::BackgroundHex("not a colour".into())
    ));
    let Some(OverlayAction::Open(spec)) = tap_overlay(&state) else {
        panic!("an open overlay");
    };
    assert!(!spec.close_on_outside_click);
    assert_eq!(spec.animation, OverlayAnimation::MoveIn(OverlayEdge::Top));
    assert_eq!(spec.background.as_deref(), Some("#FF000080"));
    assert!(state.set_selected_overlay_option(tap, &OverlayOptionEdit::BackgroundHex(" ".into())));
    let Some(OverlayAction::Open(spec)) = tap_overlay(&state) else {
        panic!("an open overlay");
    };
    assert_eq!(spec.background, None);
}

#[test]
fn swap_and_close_write_their_actions() {
    let mut state = state();
    assert!(state.set_selected_interaction_action(
        InteractionTrigger::Tap,
        &InteractionActionEdit::SwapOverlay {
            target: "sheet".into()
        }
    ));
    assert!(matches!(
        tap_overlay(&state),
        Some(OverlayAction::Swap { ref target, .. }) if target == "sheet"
    ));
    // A swap has no position.
    assert!(!state.set_selected_overlay_option(
        InteractionTrigger::Tap,
        &OverlayOptionEdit::Position(OverlayPosition::TopLeft)
    ));
    assert!(state.set_selected_interaction_action(
        InteractionTrigger::Tap,
        &InteractionActionEdit::CloseOverlay
    ));
    assert_eq!(tap_overlay(&state), Some(OverlayAction::Close));
}

fn with_tap(node: &mut PenNode, actions: &str) {
    let mut value = serde_json::to_value(&*node).expect("node json");
    value["events"] = serde_json::json!({
        "onTap": serde_json::from_str::<serde_json::Value>(actions).expect("actions json")
    });
    *node = serde_json::from_value(value).expect("node back");
}

#[test]
fn the_flow_graph_lists_overlay_links_with_targets_on_any_page() {
    use crate::interaction_graph::{flow_graph, FlowAction};
    let mut doc: PenDocument = serde_json::from_str(DOC).expect("fixture parses");
    let pages = doc.pages.as_mut().expect("pages");
    // The screen's button opens the sheet on the Overlays page; the card
    // master closes the overlay it sits in; one swap aims at nothing.
    let PenNode::Frame(home) = &mut pages[0].children[0] else {
        panic!("home is a frame");
    };
    with_tap(
        &mut home.children.as_mut().expect("children")[0],
        r##"[{"open_overlay":{"target":"sheet","position":"bottom_center","background":"#00000066"}}]"##,
    );
    with_tap(&mut pages[0].children[2], r#"[{"close_overlay":null}]"#);
    with_tap(
        &mut pages[0].children[1],
        r#"[{"swap_overlay":{"target":"gone"}}]"#,
    );

    let graph = flow_graph(&doc, Some("p1"));
    let open = graph
        .links
        .iter()
        .find(|l| l.from == "btn")
        .expect("the button's link");
    assert_eq!(open.action, FlowAction::OpenOverlay);
    assert_eq!(open.target_id.as_deref(), Some("sheet"));
    assert_eq!(open.target_page_id.as_deref(), Some("p2"));
    assert_eq!(open.target_name.as_deref(), Some("Sheet"));
    assert!(open.broken.is_none() && open.is_overlay());
    assert_eq!(
        open.overlay.as_ref().and_then(|o| o.get("position")),
        Some(&serde_json::json!("bottom_center"))
    );
    // The master's close shows on the master and once on its instance.
    let closes: Vec<&str> = graph
        .links
        .iter()
        .filter(|l| l.action == FlowAction::CloseOverlay)
        .map(|l| l.from.as_str())
        .collect();
    assert_eq!(closes, ["card", "card-1"]);
    let swap = graph
        .links
        .iter()
        .find(|l| l.action == FlowAction::SwapOverlay)
        .expect("the swap");
    assert!(swap.broken.is_some());
}
