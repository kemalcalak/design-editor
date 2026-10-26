#![cfg(test)]
//! Present replays a slide's entrances when it arrives (#245).

use super::{test_measure, PreviewSession};

fn deck() -> PreviewSession {
    let entrance = serde_json::json!([{"trigger":"mount","durationMs":100,"easing":"linear",
        "keyframes":{"from":{"opacity":0},"to":{"opacity":1}}}]);
    let doc = serde_json::json!({
        "version":"1.1","formatVersion":"1.1",
        "children":[
            {"type":"frame","id":"one","x":0,"y":0,"width":200,"height":100,"children":[
                {"type":"rectangle","id":"a","width":50,"height":50,"opacity":1,
                 "fill":[{"type":"solid","color":"#000000"}],"animations":entrance.clone()}]},
            {"type":"frame","id":"two","x":300,"y":0,"width":200,"height":100,"children":[
                {"type":"rectangle","id":"b","width":50,"height":50,"opacity":1,
                 "fill":[{"type":"solid","color":"#000000"}],"animations":entrance}]}
        ]
    });
    let doc = jian_ops_schema::load_str(&doc.to_string())
        .expect("deck fixture")
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

fn opacity(session: &PreviewSession, id: &str) -> f32 {
    session
        .preview_scene_for_test()
        .active_page()
        .and_then(|page| page.find(id))
        .expect("scene node")
        .opacity
}

#[test]
fn arriving_on_a_slide_plays_its_entrances_from_the_first_frame() {
    let mut session = deck();
    session.pump(1_000);
    assert!(
        (opacity(&session, "b") - 1.0).abs() < 0.001,
        "entrance long over"
    );

    session.replay_board_mount_animations("two", 1_000);
    session.pump(1_000);
    assert!(
        opacity(&session, "b").abs() < 0.001,
        "back at the first frame"
    );
    session.pump(1_050);
    let mid = opacity(&session, "b");
    assert!(mid > 0.0 && mid < 1.0, "mid-entrance: {mid}");
    // The slide left behind is not touched.
    assert!((opacity(&session, "a") - 1.0).abs() < 0.001);
    session.pump(1_100);
    assert!((opacity(&session, "b") - 1.0).abs() < 0.001);
}

#[test]
fn an_unknown_board_replays_nothing() {
    let mut session = deck();
    session.pump(1_000);
    session.replay_board_mount_animations("missing", 1_000);
    assert_eq!(session.active_animation_track_count(), 0);
}
