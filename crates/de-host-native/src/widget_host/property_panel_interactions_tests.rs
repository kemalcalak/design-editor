//! Host-side dispatch tests for the Interactions section's write
//! path — `apply_property_action` + `EditorCommand::PatchNodeData` +
//! undo. Hit-test / popover-row / row-parsing coverage lives in
//! `de-editor-ui`'s `property_panel_interactions_tests.rs`; these
//! tests cover what only the host can exercise (real `EditorState`
//! mutation + history).

use super::WidgetHostNative;
use de_editor_core::interaction_edit::{InteractionMenuTarget, InteractionTrigger, NavigateVerb};
use de_editor_core::NodeId;
use de_editor_ui::widgets::{PropertyPanel, PropertyPanelAction};
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

fn seed(host: &mut WidgetHostNative, json: &str) {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
}

fn node_events_json(host: &WidgetHostNative, id: &str) -> serde_json::Value {
    let node =
        de_editor_core::walkers::find_node(host.editor_state().active_children(), &NodeId::new(id))
            .expect("node present");
    serde_json::to_value(node).expect("node serializes")
}

/// Brute-force scan the property rail for a point the open Navigate/
/// Back/Remove popover maps to row `want_row` — mirrors the
/// `point_for_action` scan in `property_panel_press_tests.rs`.
fn point_for_menu_row(host: &WidgetHostNative, want_row: usize) -> Point2D {
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("property panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let mut y = rect.origin.y;
    while y < rect.origin.y + rect.size.y {
        let mut x = rect.origin.x;
        while x < rect.origin.x + rect.size.x {
            let point = Point2D::new(x, y);
            if panel.interaction_menu_row_at(rect, point) == Some(want_row) {
                return point;
            }
            x += 2.0;
        }
        y += 2.0;
    }
    panic!("no point maps to interaction-menu row {want_row}");
}

fn update_interaction_hover(host: &mut WidgetHostNative, point: Point2D) -> bool {
    let panel = PropertyPanel::for_selection(host.editor_state());
    host.update_interaction_menu_hover(point.x, point.y, VIEWPORT_W, VIEWPORT_H, panel.as_ref())
}

/// Find a point where an Effects popover row visually covers one of the
/// inspector body's action buttons. This is the exact geometry that used to
/// leave the covered button's stale hover wash visible through the popover.
fn point_where_effect_menu_overlaps_action(host: &WidgetHostNative) -> (Point2D, usize, usize) {
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("property panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let popup = panel
        .effect_add_menu_rect(rect)
        .expect("open effect menu has painted bounds");
    // Scan the popup rather than assume its centre: the overlap depends on
    // how many effect kinds the menu lists.
    let mut y = popup.origin.y;
    while y < popup.origin.y + popup.size.y {
        let mut x = popup.origin.x;
        while x < popup.origin.x + popup.size.x {
            let point = Point2D::new(x, y);
            if let (Some(row), Some(action)) = (
                panel.effect_add_menu_row_at(rect, point),
                panel.action_hover_index(rect, point),
            ) {
                return (point, action, row);
            }
            x += 4.0;
        }
        y += 4.0;
    }
    panic!("no effect-menu row overlaps an inspector action");
}

#[test]
fn toggle_interaction_menu_opens_and_closes() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
            {"type":"rectangle","id":"f1","name":"Rectangle",
             "width":180,"height":120,
             "fill":[{"type":"solid","color":"#BDC7D9"}]}
        ]}"##,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    assert!(!host.editor_state().editor_ui.interaction_menu_open);

    let add = PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::AddTrigger);
    host.apply_property_action(add.clone());
    assert!(host.editor_state().editor_ui.interaction_menu_open);

    host.apply_property_action(add);
    assert!(!host.editor_state().editor_ui.interaction_menu_open);
}

#[test]
fn set_interaction_navigate_writes_the_double_encoded_patch() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame"}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));

    host.apply_property_action(PropertyPanelAction::SetInteractionNavigate {
        trigger: InteractionTrigger::Tap,
        path: "/profile".to_string(),
    });

    let value = node_events_json(&host, "f1");
    assert_eq!(
        value["events"]["onTap"][0]["replace"],
        serde_json::Value::String("\"/profile\"".to_string())
    );
}

#[test]
fn set_interaction_navigate_then_undo_restores_no_events() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame"}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));

    host.apply_property_action(PropertyPanelAction::SetInteractionNavigate {
        trigger: InteractionTrigger::Tap,
        path: "/profile".to_string(),
    });
    assert!(node_events_json(&host, "f1").get("events").is_some());

    assert!(host.editor_state_mut().undo(), "one undo step available");
    let restored = node_events_json(&host, "f1");
    assert!(
        restored.get("events").is_none(),
        "undo should restore the pre-edit (no events) node"
    );
}

#[test]
fn remove_interaction_clears_on_tap_without_leaving_an_empty_events_shell() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame",
             "events":{"onTap":[{"pop":null}]}}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));

    host.apply_property_action(PropertyPanelAction::RemoveInteraction(
        InteractionTrigger::Tap,
    ));

    let value = node_events_json(&host, "f1");
    assert!(
        value.get("events").is_none(),
        "onTap was the only handler — events must serialize away entirely, not as `events:{{}}`"
    );
}

#[test]
fn remove_interaction_preserves_a_sibling_event_handler() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"text_input","id":"in1","name":"Input",
             "events":{"onTap":[{"pop":null}],"onChange":[{"set":{"$state.x":"1"}}]}}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("in1"));

    host.apply_property_action(PropertyPanelAction::RemoveInteraction(
        InteractionTrigger::Tap,
    ));

    let value = node_events_json(&host, "in1");
    assert!(
        value["events"]["onTap"].is_null() || value["events"].get("onTap").is_none(),
        "onTap should be cleared"
    );
    assert!(
        value["events"]["onChange"].is_array(),
        "a sibling handler must survive the Remove"
    );
}

#[test]
fn set_interaction_pop_writes_the_pop_action() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame"}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));

    host.apply_property_action(PropertyPanelAction::SetInteractionPop(
        InteractionTrigger::Tap,
    ));

    let value = node_events_json(&host, "f1");
    assert!(value["events"]["onTap"][0]["pop"].is_null());
}

#[test]
fn interaction_menu_hover_tracks_the_row_under_the_cursor() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame"}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    let property_rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    assert!(host.apply_wheel(
        property_rect.origin.x + property_rect.size.x / 2.0,
        property_rect.origin.y + property_rect.size.y / 2.0,
        -10_000.0,
        VIEWPORT_W,
        VIEWPORT_H,
    ));
    host.apply_property_action(PropertyPanelAction::ToggleInteractionMenu(
        InteractionMenuTarget::Trigger(InteractionTrigger::Tap),
    ));
    assert!(host.editor_state().editor_ui.interaction_menu_open);
    assert_eq!(host.editor_state().editor_ui.interaction_menu_hover, None);

    // No authored screens on this doc — row 0 is "Back (pop)".
    let point = point_for_menu_row(&host, 0);
    assert!(update_interaction_hover(&mut host, point));
    assert_eq!(
        host.editor_state().editor_ui.interaction_menu_hover,
        Some(0)
    );

    // A second update at the same point is a no-op (hover unchanged).
    assert!(!update_interaction_hover(&mut host, point));

    // Moving off every row clears the hover.
    assert!(update_interaction_hover(
        &mut host,
        Point2D::new(-500.0, -500.0)
    ));
    assert_eq!(host.editor_state().editor_ui.interaction_menu_hover, None);
}

#[test]
fn interaction_menu_hover_is_a_no_op_while_closed() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame"}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    assert!(!host.editor_state().editor_ui.interaction_menu_open);

    assert!(!update_interaction_hover(&mut host, Point2D::new(0.0, 0.0)));
}

#[test]
fn effect_popover_hover_clears_the_covered_inspector_action_hover() {
    let mut host = WidgetHostNative::new();
    // A tap action gives Interactions a full-width row for the effect
    // menu to cover (an empty Interactions section is just its header).
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame","events":{"onTap":[{"pop":null}]}}
        ]}"#,
    );
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    host.apply_property_action(PropertyPanelAction::ToggleEffectAddPicker);

    let (point, covered_action, effect_row) = point_where_effect_menu_overlaps_action(&host);
    host.editor_state_mut().editor_ui.property_action_hover = Some(covered_action);

    assert!(host.apply_cursor_move(point.x, point.y));
    assert_eq!(
        host.editor_state().editor_ui.effect_add_menu_hover,
        Some(effect_row),
        "the top popover row should own hover"
    );
    assert_eq!(
        host.editor_state().editor_ui.property_action_hover,
        None,
        "the covered inspector action must not retain a hover wash"
    );

    // Even when the popover's own row is unchanged, it remains an owning
    // floating surface and must clear any stale lower hover before returning.
    host.editor_state_mut().editor_ui.property_action_hover = Some(covered_action);
    assert!(host.apply_cursor_move(point.x, point.y));
    assert_eq!(host.editor_state().editor_ui.property_action_hover, None);
}

fn seeded_frame() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame","width":200,"height":120}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    host
}

#[test]
fn picking_a_trigger_keeps_the_popover_open_on_its_actions() {
    let mut host = seeded_frame();
    host.apply_property_action(PropertyPanelAction::ToggleInteractionMenu(
        InteractionMenuTarget::AddTrigger,
    ));
    host.apply_property_action(PropertyPanelAction::PickInteractionTrigger(
        InteractionTrigger::Hover,
    ));
    let ui = &host.editor_state().editor_ui;
    assert!(ui.interaction_menu_open);
    assert_eq!(
        ui.interaction_menu_target,
        InteractionMenuTarget::Trigger(InteractionTrigger::Hover)
    );
    assert!(
        node_events_json(&host, "f1").get("events").is_none(),
        "picking a trigger writes nothing yet"
    );
    // The pending trigger's popover still has a place to drop from.
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    assert!(panel.interaction_menu_anchor_rect(rect).is_some());

    host.apply_property_action(PropertyPanelAction::SetInteractionPop(
        InteractionTrigger::Hover,
    ));
    assert!(!host.editor_state().editor_ui.interaction_menu_open);
    assert!(node_events_json(&host, "f1")["events"]["onHoverEnter"][0]["pop"].is_null());
}

#[test]
fn long_press_navigate_and_transition_are_one_undo_step_each() {
    let mut host = seeded_frame();
    let trigger = InteractionTrigger::LongPress;
    host.apply_property_action(PropertyPanelAction::SetInteractionNavigate {
        trigger,
        path: "/next".to_string(),
    });
    host.apply_property_action(PropertyPanelAction::SetInteractionTransition {
        trigger,
        verb: NavigateVerb::Push,
    });
    let value = node_events_json(&host, "f1");
    assert_eq!(
        value["events"]["onLongPress"][0]["push"],
        serde_json::Value::String("\"/next\"".to_string())
    );
    assert!(host.editor_state_mut().undo());
    assert!(node_events_json(&host, "f1")["events"]["onLongPress"][0]["replace"].is_string());
    assert!(host.editor_state_mut().undo());
    assert!(node_events_json(&host, "f1").get("events").is_none());
}

#[test]
fn a_tap_edit_keeps_a_sibling_handler() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame",
             "events":{"onSwipe":[{"pop":null}]}}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    host.apply_property_action(PropertyPanelAction::SetInteractionPop(
        InteractionTrigger::Tap,
    ));
    let value = node_events_json(&host, "f1");
    assert!(value["events"]["onTap"].is_array());
    assert!(
        value["events"]["onSwipe"].is_array(),
        "the old onTap patch replaced the whole events block"
    );
}

#[test]
fn motion_add_trigger_property_and_remove_round_trip_with_undo() {
    let mut host = seeded_frame();
    host.apply_property_action(PropertyPanelAction::AddAnimation);
    host.apply_property_action(PropertyPanelAction::SetAnimationTrigger {
        index: 0,
        trigger: jian_ops_schema::motion::MotionTrigger::InView,
    });
    host.apply_property_action(PropertyPanelAction::SetAnimationProperty {
        index: 0,
        property: de_editor_core::motion_edit::MotionProperty::TranslateY,
    });
    let value = node_events_json(&host, "f1");
    assert_eq!(value["animations"][0]["trigger"], "inView");
    assert!(value["animations"][0]["keyframes"][0]["values"]["translateY"].is_number());

    host.apply_property_action(PropertyPanelAction::RemoveAnimation(0));
    assert!(node_events_json(&host, "f1").get("animations").is_none());
    assert!(host.editor_state_mut().undo());
    assert!(node_events_json(&host, "f1")["animations"].is_array());
}

#[test]
fn a_motion_value_focus_hits_and_commits() {
    let mut host = seeded_frame();
    host.apply_property_action(PropertyPanelAction::AddAnimation);
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let rects = panel.motion_input_rects(rect);
    assert_eq!(rects.len(), 4);
    let focus = de_editor_core::PropertyFocus::MotionValue {
        index: 0,
        field: de_editor_core::motion_edit::MotionField::DurationMs,
    };
    let (_, duration) = rects.iter().find(|(f, _)| *f == focus).expect("duration");
    let centre = Point2D::new(
        duration.origin.x + duration.size.x / 2.0,
        duration.origin.y + duration.size.y / 2.0,
    );
    if rect.contains(centre) {
        assert_eq!(panel.hit_test(rect, centre), Some(focus));
    }
    assert_eq!(
        de_editor_ui::widgets::property_panel_commit::property_focus_initial(focus, &panel),
        "300"
    );
    let state = host.editor_state_mut();
    state.ui.property_focus = Some(focus);
    state.ui.property_input.set_text("750");
    assert!(de_editor_ui::widgets::property_panel_commit::commit_property_focus(state));
    assert_eq!(
        node_events_json(&host, "f1")["animations"][0]["durationMs"],
        750
    );
    assert!(host.editor_state_mut().undo());
    assert_eq!(
        node_events_json(&host, "f1")["animations"][0]["durationMs"],
        300
    );
}

/// A point on the panel that maps to `want` and lies outside the open
/// popover.
fn point_for_action_outside_menu(host: &WidgetHostNative, want: &PropertyPanelAction) -> Point2D {
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("property panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    let mut y = rect.origin.y;
    while y < rect.origin.y + rect.size.y {
        let mut x = rect.origin.x;
        while x < rect.origin.x + rect.size.x {
            let point = Point2D::new(x, y);
            if panel.hit_test_action(rect, point).as_ref() == Some(want)
                && matches!(
                    panel.interaction_menu_hit(rect, point),
                    de_editor_ui::widgets::InteractionMenuHit::Outside
                )
            {
                return point;
            }
            x += 3.0;
        }
        y += 3.0;
    }
    panic!("no point maps to {want:?} outside the popover");
}

/// #176: with one row's popover open, a press on another row switches
/// the popover to that row in one click (it used to only close it).
#[test]
fn a_press_on_another_row_switches_the_popover_in_one_click() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{ "version": "1.0.0", "children": [
            {"type":"frame","id":"f1","name":"Frame","width":200,"height":120,
             "events":{"onTap":[{"pop":null}],"onHoverEnter":[{"pop":null}]}}
        ]}"#,
    );
    host.editor_state_mut()
        .set_single_selection(NodeId::new("f1"));
    let tap = InteractionMenuTarget::Trigger(InteractionTrigger::Tap);
    let hover = InteractionMenuTarget::Trigger(InteractionTrigger::Hover);
    // The Hover row's popover drops below it, so the Tap row above
    // stays clickable.
    host.apply_property_action(PropertyPanelAction::ToggleInteractionMenu(hover));
    assert!(host.editor_state().editor_ui.interaction_menu_open);

    let point =
        point_for_action_outside_menu(&host, &PropertyPanelAction::ToggleInteractionMenu(tap));
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    let ui = &host.editor_state().editor_ui;
    assert!(ui.interaction_menu_open, "the popover stays open");
    assert_eq!(ui.interaction_menu_target, tap);

    // Its own row closes it, also in one click.
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(!host.editor_state().editor_ui.interaction_menu_open);
}

#[test]
fn open_url_and_transition_edits_round_trip_with_undo() {
    let mut host = seeded_frame();
    let tap = InteractionTrigger::Tap;
    host.apply_property_action(PropertyPanelAction::SetInteractionOpenUrl(tap));
    let state = host.editor_state_mut();
    let focus = de_editor_core::PropertyFocus::InteractionUrl(tap);
    state.ui.property_focus = Some(focus);
    state.ui.property_input.set_text("https://example.com");
    assert!(de_editor_ui::widgets::property_panel_commit::commit_property_focus(state));
    assert_eq!(
        node_events_json(&host, "f1")["events"]["onTap"][0]["open_url"]["url"],
        "\"https://example.com\""
    );

    host.apply_property_action(PropertyPanelAction::MoveInteraction {
        from: tap,
        to: InteractionTrigger::LongPress,
    });
    let json = node_events_json(&host, "f1");
    assert!(json["events"]["onTap"].is_null());
    assert!(json["events"]["onLongPress"][0]["open_url"].is_object());

    host.apply_property_action(PropertyPanelAction::AddTransition);
    host.apply_property_action(PropertyPanelAction::SetTransitionEasing(
        de_editor_core::transition_edit::MotionEasing::Linear,
    ));
    assert_eq!(
        node_events_json(&host, "f1")["transition"]["easing"],
        "linear"
    );
    let panel = PropertyPanel::for_selection(host.editor_state()).expect("panel");
    let rect = host.property_rect(VIEWPORT_W, VIEWPORT_H);
    assert!(panel
        .motion_input_rects(rect)
        .iter()
        .any(|(f, _)| *f == de_editor_core::PropertyFocus::TransitionDuration));
    assert!(host.editor_state_mut().undo());
    assert_eq!(
        node_events_json(&host, "f1")["transition"]["easing"],
        "standard"
    );
    host.apply_property_action(PropertyPanelAction::RemoveTransition);
    assert!(node_events_json(&host, "f1").get("transition").is_none());
}
