//! Scripted capture runs (#156).

use super::*;
use crate::test_measure;
use crate::tests_app_mode::TWO_SCREEN_DOC_JSON;

fn session(json: &str) -> PreviewSession {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(json).expect("fixture");
    let mut session = PreviewSession::enter(
        &doc,
        (1200.0, 800.0),
        &Default::default(),
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

fn shown_root(scene: &LayoutScene) -> String {
    scene
        .active_page()
        .and_then(|page| page.children.first())
        .map(|node| node.id.clone())
        .expect("a mounted root")
}

#[test]
fn a_tap_on_a_navigate_button_shows_the_next_screen() {
    let mut session = session(TWO_SCREEN_DOC_JSON);
    let scene = session
        .run_capture_script(&[CaptureStep::Tap("go".into())], 0)
        .expect("script runs");
    assert_eq!(shown_root(&scene), "detail");
    // Back returns to the entry screen.
    let scene = session
        .run_capture_script(&[CaptureStep::Back], 0)
        .expect("back runs");
    assert_eq!(shown_root(&scene), "home");
}

#[test]
fn navigate_switches_screens_and_unknown_nodes_are_reported() {
    let mut session = session(TWO_SCREEN_DOC_JSON);
    let scene = session
        .run_capture_script(&[CaptureStep::Navigate("/detail".into())], 0)
        .expect("navigate runs");
    assert_eq!(shown_root(&scene), "detail");
    assert_eq!(
        session.run_capture_script(&[CaptureStep::Tap("go".into())], 0),
        Err(CaptureStepError::NodeNotShown("go".into())),
        "the home button is not on the detail screen"
    );
    assert_eq!(
        session.run_capture_script(&[CaptureStep::Navigate("/nowhere".into())], 0),
        Err(CaptureStepError::UnknownScreen("/nowhere".into()))
    );
    assert_eq!(
        session.run_capture_script(&[CaptureStep::Back], 0),
        Err(CaptureStepError::NothingToGoBackTo),
        "navigate replaces: there is no earlier screen"
    );
}

#[test]
fn waits_let_mount_motion_finish_and_long_scripts_are_refused() {
    let json = r##"{"version":"1.1","formatVersion":"1.1","children":[
        {"type":"rectangle","id":"hero","width":100,"height":100,"opacity":1,
         "fill":[{"type":"solid","color":"#ffffff"}],
         "animations":[{"trigger":"mount","durationMs":300,"easing":"linear",
            "keyframes":{"from":{"opacity":0},"to":{"opacity":1}}}]}]}"##;
    let mut early = session(json);
    let scene = early.run_capture_script(&[], 0).expect("runs");
    let opacity = |scene: &LayoutScene| {
        scene
            .active_page()
            .and_then(|page| page.find("hero"))
            .map(|node| node.opacity)
            .expect("hero")
    };
    assert!(opacity(&scene) < 0.2, "captured at the start of the fade");
    let mut late = session(json);
    let scene = late
        .run_capture_script(&[CaptureStep::Wait(200)], 200)
        .expect("runs");
    assert!(
        (opacity(&scene) - 1.0).abs() < 0.001,
        "the fade has finished"
    );

    assert_eq!(
        late.run_capture_script(&[CaptureStep::Wait(MAX_CAPTURE_MS)], 1),
        Err(CaptureStepError::TooLong)
    );
    assert_eq!(
        late.run_capture_script(&[CaptureStep::Back], 0),
        Err(CaptureStepError::NotAppMode)
    );
}
