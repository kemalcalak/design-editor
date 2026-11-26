use super::*;

fn action(json: &str) -> Action {
    serde_json::from_str(json).expect("action parses")
}

#[test]
fn open_overlay_parses_every_option() {
    let parsed = OverlayAction::parse(&action(
        r##"{"open_overlay":{"target":"n50","position":"bottom_center","closeOnOutsideClick":false,
            "background":"#00000066","animation":"move_in","direction":"bottom","duration":250}}"##,
    ))
    .expect("an overlay action");
    let OverlayAction::Open(spec) = parsed else {
        panic!("an open");
    };
    assert_eq!(spec.target, "n50");
    assert_eq!(spec.position, OverlayPosition::BottomCenter);
    assert!(!spec.close_on_outside_click);
    assert_eq!(spec.background.as_deref(), Some("#00000066"));
    assert_eq!(spec.animation, OverlayAnimation::MoveIn(OverlayEdge::Bottom));
    assert_eq!(spec.duration_ms, 250);
    let rgba = spec.background_rgba().expect("a dim");
    assert!((rgba[3] - 0.4).abs() < 0.01);
}

#[test]
fn a_bare_open_takes_the_defaults() {
    let Some(OverlayAction::Open(spec)) =
        OverlayAction::parse(&action(r#"{"open_overlay":{"target":"menu"}}"#))
    else {
        panic!("an open");
    };
    assert_eq!(spec, OverlaySpec::new("menu"));
    assert!(spec.close_on_outside_click);
    assert_eq!(spec.background, None);
}

#[test]
fn an_open_without_a_target_is_not_an_overlay() {
    assert_eq!(
        OverlayAction::parse(&action(r#"{"open_overlay":{"position":"center"}}"#)),
        None
    );
    assert_eq!(
        OverlayAction::parse(&action(r#"{"push":"\"/a\""}"#)),
        None
    );
}

#[test]
fn every_action_round_trips_through_its_stored_shape() {
    let mut spec = OverlaySpec::new("sheet");
    spec.position = OverlayPosition::Manual;
    spec.offset = (4.0, 8.0);
    spec.background = Some(DEFAULT_OVERLAY_DIM.to_owned());
    spec.animation = OverlayAnimation::MoveIn(OverlayEdge::Left);
    spec.duration_ms = 450;
    for overlay in [
        OverlayAction::Open(spec),
        OverlayAction::Open(OverlaySpec::new("plain")),
        OverlayAction::Swap {
            target: "next".into(),
            animation: OverlayAnimation::Dissolve,
            duration_ms: DEFAULT_OVERLAY_DURATION_MS,
        },
        OverlayAction::Close,
    ] {
        assert_eq!(OverlayAction::parse(&overlay.to_action()), Some(overlay));
    }
}

#[test]
fn the_stored_shape_leaves_defaults_out() {
    let body = OverlayAction::Open(OverlaySpec::new("m")).body();
    assert_eq!(
        body,
        serde_json::json!({"target":"m","position":"center","closeOnOutsideClick":true})
    );
    assert_eq!(OverlayAction::Close.body(), Value::Null);
}

#[test]
fn positions_accept_their_spellings() {
    for position in OverlayPosition::ALL {
        assert_eq!(
            OverlayPosition::from_wire(position.wire_name()),
            Some(position)
        );
    }
    assert_eq!(
        OverlayPosition::from_wire("topRight"),
        Some(OverlayPosition::TopRight)
    );
    assert_eq!(
        OverlayPosition::from_wire("bottom-left"),
        Some(OverlayPosition::BottomLeft)
    );
    assert_eq!(OverlayPosition::from_wire("middle"), None);
}

#[test]
fn placement_anchors_inside_the_viewport_or_follows_the_trigger() {
    let viewport = (0.0, 100.0, 400.0, 800.0);
    let size = (200.0, 100.0);
    assert_eq!(
        OverlayPosition::Center.place(viewport, size, None, (0.0, 0.0)),
        (100.0, 450.0)
    );
    assert_eq!(
        OverlayPosition::BottomRight.place(viewport, size, None, (0.0, 0.0)),
        (200.0, 800.0)
    );
    assert_eq!(
        OverlayPosition::TopLeft.place(viewport, size, None, (0.0, 0.0)),
        (0.0, 100.0)
    );
    assert_eq!(
        OverlayPosition::Manual.place(viewport, size, Some((30.0, 60.0)), (5.0, 2.0)),
        (35.0, 62.0)
    );
}

#[test]
fn navigate_transition_names_read_as_overlay_animations() {
    assert_eq!(
        OverlayAnimation::from_wire("fade", None),
        Some(OverlayAnimation::Dissolve)
    );
    assert_eq!(
        OverlayAnimation::from_wire("slide", Some("top")),
        Some(OverlayAnimation::MoveIn(OverlayEdge::Top))
    );
    assert_eq!(OverlayAnimation::from_wire("spin", None), None);
}
