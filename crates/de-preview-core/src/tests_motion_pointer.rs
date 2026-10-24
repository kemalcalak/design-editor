//! Hover and tap motion triggers, and size / colour / radius tweens (#155).

use super::{test_measure, PreviewSession};
use crate::InvalidationKind;
use de_editor_ui::layout_scene::SceneNode;
use jian_core::gesture::pointer::PointerPhase;

/// A 300×200 frame with one 100×40 button at (20, 20) carrying `animation`.
fn session_with(animation: serde_json::Value) -> PreviewSession {
    let source = serde_json::json!({
        "version":"1.1","formatVersion":"1.1",
        "children":[{"type":"frame","id":"root","width":300,"height":200,"children":[
            {"type":"rectangle","id":"btn","x":20,"y":20,"width":100,"height":40,
             "opacity":1,"cornerRadius":0,
             "fill":[{"type":"solid","color":"#ff0000"}],
             "animations":[animation]}]}]
    });
    let doc = jian_ops_schema::load_str(&source.to_string())
        .expect("motion fixture")
        .value;
    let mut session = PreviewSession::enter(
        &doc,
        (400.0, 400.0),
        &std::collections::BTreeMap::new(),
        0,
        false,
        false,
        test_measure(),
        0,
    )
    .expect("preview session");
    session.begin_lifecycle(0);
    session
}

fn btn(session: &PreviewSession) -> SceneNode {
    session
        .preview_scene_for_test()
        .active_page()
        .and_then(|page| page.find("btn"))
        .cloned()
        .expect("btn")
}

fn pointer(session: &mut PreviewSession, x: f32, y: f32, phase: PointerPhase, t: u64) {
    let _ = session.dispatch_pointer_phase_at(x, y, phase, t);
}

fn tween(trigger: &str, from: serde_json::Value, to: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"trigger":trigger,"durationMs":100,"easing":"linear",
        "keyframes":{"from":from,"to":to}})
}

#[test]
fn hover_plays_forward_on_enter_and_back_on_leave() {
    let mut session = session_with(tween(
        "hover",
        serde_json::json!({"opacity":1}),
        serde_json::json!({"opacity":0.5}),
    ));
    assert_eq!(session.active_animation_track_count(), 0, "nothing at rest");
    pointer(&mut session, 70.0, 40.0, PointerPhase::Hover, 0);
    session.pump(50);
    assert!((btn(&session).opacity - 0.75).abs() < 0.01);
    session.pump(100);
    assert!((btn(&session).opacity - 0.5).abs() < 0.01);

    // Leaving plays back to the authored opacity.
    pointer(&mut session, 250.0, 150.0, PointerPhase::Hover, 200);
    session.pump(250);
    assert!((btn(&session).opacity - 0.75).abs() < 0.01);
    session.pump(300);
    assert!((btn(&session).opacity - 1.0).abs() < 0.01);
}

#[test]
fn leaving_mid_way_reverses_from_where_it_was() {
    let mut session = session_with(tween(
        "hover",
        serde_json::json!({"opacity":1}),
        serde_json::json!({"opacity":0}),
    ));
    pointer(&mut session, 70.0, 40.0, PointerPhase::Hover, 0);
    session.pump(40);
    let mid = btn(&session).opacity;
    assert!((mid - 0.6).abs() < 0.01);
    assert!(session.clear_hover());
    session.pump(40);
    assert!(
        (btn(&session).opacity - mid).abs() < 0.01,
        "no jump on leave"
    );
    session.pump(140);
    assert!((btn(&session).opacity - 1.0).abs() < 0.01);
}

#[test]
fn tap_plays_forward_on_press_and_back_on_release() {
    let mut session = session_with(tween(
        "tap",
        serde_json::json!({"fill":"#ff0000"}),
        serde_json::json!({"fill":"#0000ff"}),
    ));
    pointer(&mut session, 70.0, 40.0, PointerPhase::Down, 0);
    session.pump(50);
    let fill = btn(&session).fill.expect("fill");
    assert!((fill.r - 0.5).abs() < 0.02 && (fill.b - 0.5).abs() < 0.02);
    session.pump(100);
    let fill = btn(&session).fill.expect("fill");
    assert!(fill.b > 0.98 && fill.r < 0.02);

    pointer(&mut session, 70.0, 40.0, PointerPhase::Up, 120);
    session.pump(220);
    let fill = btn(&session).fill.expect("fill");
    assert!(fill.r > 0.98 && fill.b < 0.02, "released: back to red");
}

#[test]
fn width_and_height_tween_every_frame_and_only_repaint() {
    let mut session = session_with(tween(
        "hover",
        serde_json::json!({"width":100,"height":40}),
        serde_json::json!({"width":200,"height":80}),
    ));
    pointer(&mut session, 70.0, 40.0, PointerPhase::Hover, 0);
    session.pump(50);
    let size = btn(&session).bounds.size;
    assert!((size.x - 150.0).abs() < 0.5, "{size:?}");
    assert!((size.y - 60.0).abs() < 0.5, "{size:?}");
    assert_eq!(
        session.last_animation_invalidation(),
        InvalidationKind::PaintOnly,
        "a size tween does not relayout every frame"
    );
    assert_eq!(session.next_wake_deadline_ms(), Some(66), "a 60 Hz wake");
}

#[test]
fn corner_radius_tweens_on_tap() {
    let mut session = session_with(tween(
        "tap",
        serde_json::json!({"cornerRadius":0}),
        serde_json::json!({"cornerRadius":20}),
    ));
    pointer(&mut session, 70.0, 40.0, PointerPhase::Down, 0);
    session.pump(50);
    assert!((btn(&session).corner_radius - 10.0).abs() < 0.1);
}

#[test]
fn reduced_motion_jumps_straight_to_the_end_values() {
    let mut session = session_with(tween(
        "hover",
        serde_json::json!({"opacity":1}),
        serde_json::json!({"opacity":0.5}),
    ));
    session.set_motion_preference(jian_ops_schema::motion::MotionPreference::Reduced);
    pointer(&mut session, 70.0, 40.0, PointerPhase::Hover, 0);
    assert!((btn(&session).opacity - 0.5).abs() < 0.01);
    assert!(session.clear_hover());
    assert!((btn(&session).opacity - 1.0).abs() < 0.01);
}

#[test]
fn pressing_a_hovered_node_keeps_its_hover_animation() {
    let mut session = session_with(tween(
        "hover",
        serde_json::json!({"opacity":1}),
        serde_json::json!({"opacity":0.5}),
    ));
    pointer(&mut session, 70.0, 40.0, PointerPhase::Hover, 0);
    session.pump(100);
    pointer(&mut session, 70.0, 40.0, PointerPhase::Down, 120);
    session.pump(200);
    assert!(
        (btn(&session).opacity - 0.5).abs() < 0.01,
        "a press does not end the hover"
    );
    pointer(&mut session, 70.0, 40.0, PointerPhase::Up, 220);
    session.pump(300);
    assert!((btn(&session).opacity - 0.5).abs() < 0.01);
}
