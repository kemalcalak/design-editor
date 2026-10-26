//! The imported entrances play in Present: arriving on the animated slide
//! replays them from their first frame, in their PowerPoint order.

use de_preview_core::PreviewSession;

use super::fixture;
use crate::import_pptx;

fn opacity(session: &PreviewSession, id: &str) -> f32 {
    session
        .preview_scene_for_test()
        .active_page()
        .and_then(|page| page.find(id))
        .unwrap_or_else(|| panic!("{id} in the scene"))
        .opacity
}

fn id_of(doc: &serde_json::Value, name: &str) -> String {
    super::find(super::boards(doc)[3], name)
        .and_then(|n| n["id"].as_str())
        .unwrap_or_else(|| panic!("{name}"))
        .to_string()
}

#[test]
fn imported_entrances_play_in_order_when_the_slide_is_presented() {
    let import = import_pptx(&fixture::deck(), "Fixture").expect("fixture imports");
    let value = serde_json::to_value(&import.document).expect("serialize");
    let (title, floated) = (id_of(&value, "Motion title"), id_of(&value, "Float box"));
    let mut session = PreviewSession::enter(
        &import.document,
        (1280.0, 720.0),
        &std::collections::BTreeMap::new(),
        0,
        false,
        true,
        de_pen_loader::layout_measure_backend(),
        0,
    )
    .expect("preview session");
    session.begin_lifecycle(0);
    session.pump(5_000);

    // Present arrives on the animated slide at t = 10 s.
    session.replay_board_mount_animations("s4", 10_000);
    session.pump(10_000);
    assert!(opacity(&session, &title) < 0.01, "title starts hidden");
    session.pump(10_250);
    let mid = opacity(&session, &title);
    assert!(mid > 0.1 && mid < 0.9, "title mid-fade: {mid}");
    // The box waits for the title (500 ms) before floating in.
    assert!(opacity(&session, &floated) < 0.01, "box still waiting");
    session.pump(10_900);
    let box_mid = opacity(&session, &floated);
    assert!(
        box_mid > 0.1 && box_mid < 0.9,
        "box mid-entrance: {box_mid}"
    );
    session.pump(12_000);
    assert!((opacity(&session, &title) - 1.0).abs() < 0.01);
    assert!((opacity(&session, &floated) - 1.0).abs() < 0.01);
}
