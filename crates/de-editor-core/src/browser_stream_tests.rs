use super::*;

fn candidate(node_id: &str, screen_area: f32, selected: bool) -> LiveCandidate<'_> {
    LiveCandidate {
        node_id,
        screen_area,
        selected,
        interactive: false,
    }
}

#[test]
fn only_selected_nodes_stream() {
    let candidates = [
        candidate("big", 900.0, false),
        candidate("picked", 10.0, true),
        candidate("other", 400.0, false),
    ];
    assert_eq!(pick_live_pages(&candidates, MAX_LIVE_PAGES), ["picked"]);
    assert!(pick_live_pages(&[candidate("idle", 900.0, false)], 3).is_empty());
}

#[test]
fn a_multi_selection_streams_at_most_three_the_largest_first() {
    let candidates = [
        candidate("small", 100.0, true),
        candidate("big", 900.0, true),
        candidate("mid", 400.0, true),
        candidate("tiny", 1.0, true),
    ];
    assert_eq!(
        pick_live_pages(&candidates, MAX_LIVE_PAGES),
        ["big", "mid", "small"]
    );
    let ties = [candidate("b", 50.0, true), candidate("a", 50.0, true)];
    assert_eq!(pick_live_pages(&ties, 3), ["a", "b"], "equal sizes by id");
}

#[test]
fn an_interactive_node_is_live_and_comes_first() {
    let interactive = LiveCandidate {
        interactive: true,
        ..candidate("driven", 10.0, false)
    };
    assert!(interactive.wants_live());
    let candidates = [
        candidate("big", 900.0, true),
        candidate("mid", 400.0, true),
        candidate("small", 100.0, true),
        interactive,
    ];
    assert_eq!(
        pick_live_pages(&candidates, MAX_LIVE_PAGES),
        ["driven", "big", "mid"]
    );
}

#[test]
fn empty_boxes_never_stream() {
    let candidates = [
        candidate("gone", 0.0, true),
        candidate("nan", f32::NAN, true),
    ];
    assert!(pick_live_pages(&candidates, 3).is_empty());
    assert!(pick_live_pages(&[candidate("a", 5.0, true)], 0).is_empty());
}

#[test]
fn frame_width_steps_by_powers_of_two_within_the_bounds() {
    let max = MAX_STREAM_WIDTH;
    assert_eq!(stream_frame_width(None, 10.0, 1.0, max), MIN_STREAM_WIDTH);
    assert_eq!(stream_frame_width(None, 200.0, 1.0, max), 256);
    assert_eq!(stream_frame_width(None, 200.0, 2.0, max), 512);
    assert_eq!(stream_frame_width(None, 600.0, 2.0, max), 2048);
    assert_eq!(stream_frame_width(None, 5000.0, 2.0, max), max);
    assert_eq!(stream_frame_width(None, 5000.0, 2.0, 1024), 1024);
    assert_eq!(
        stream_frame_width(None, f32::NAN, 2.0, max),
        MIN_STREAM_WIDTH
    );
    assert_eq!(
        stream_frame_width(None, 300.0, 0.0, max),
        512,
        "a bad dpi reads as 1"
    );
}

#[test]
fn zooming_restarts_only_at_a_step_and_shrinks_with_slack() {
    let max = MAX_STREAM_WIDTH;
    // Grows as soon as the node outgrows its step.
    assert_eq!(stream_frame_width(Some(256), 257.0, 1.0, max), 512);
    // Small zoom changes keep the stream as it is.
    let mut width = stream_frame_width(None, 300.0, 1.0, max);
    for screen in [290.0, 310.0, 280.0, 320.0, 260.0] {
        width = stream_frame_width(Some(width), screen, 1.0, max);
        assert_eq!(width, 512, "at {screen}");
    }
    // Just under the smaller step is not enough to shrink…
    assert_eq!(stream_frame_width(Some(512), 250.0, 1.0, max), 512);
    // …clearly inside it is.
    assert_eq!(stream_frame_width(Some(512), 200.0, 1.0, max), 256);
    assert_eq!(
        stream_frame_width(Some(2048), 20.0, 1.0, max),
        MIN_STREAM_WIDTH
    );
    // A lower cap applies at once.
    assert_eq!(stream_frame_width(Some(2048), 1500.0, 2.0, 1024), 1024);
}

#[test]
fn hosts_cap_rate_and_size() {
    let desktop = stream_tier(StreamHost::Desktop);
    assert_eq!(desktop.max_fps, 30.0);
    assert_eq!(
        (desktop.max_width, desktop.max_height),
        (MAX_STREAM_WIDTH, MAX_STREAM_HEIGHT)
    );
    // A selected node on the web runs at 10 fps, the daemon's cap.
    assert_eq!(stream_tier(StreamHost::Web).max_fps, 10.0);
    assert_eq!(WEB_STREAM_FPS, crate::browser_live_wire::WEB_LIVE_MAX_FPS);
}

#[test]
fn a_stream_spec_keeps_the_snapshot_layout() {
    let node: jian_ops_schema::node::PenNode = serde_json::from_str(
        r#"{"type":"browser","id":"b","width":390,"height":844,"url":"https://example.com","deviceId":"phone"}"#,
    )
    .unwrap();
    let request = crate::browser_live::browser_render_request(&node, 0, None).unwrap();
    let tier = stream_tier(StreamHost::Desktop);
    let spec = BrowserStreamSpec::for_request(&request, 512, tier);
    assert_eq!((spec.layout_width, spec.layout_height), (390, 844));
    assert!(spec.mobile);
    assert_eq!(
        (spec.frame_width, spec.frame_height),
        (512, MAX_STREAM_HEIGHT)
    );
    assert_eq!(spec.page_scale, 1.5, "512 / 390 in quarter steps");
    assert_eq!(spec.max_fps, DESKTOP_STREAM_FPS);
    let wider = BrowserStreamSpec::for_request(&request, 2048, tier);
    assert!(spec.same_page(&wider));
    assert_eq!(wider.page_scale, MAX_STREAM_PAGE_SCALE);
    assert_eq!(
        BrowserStreamSpec::for_request(&request, 9999, tier).frame_width,
        MAX_STREAM_WIDTH
    );
    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("\"frameWidth\":512"), "{json}");
    assert!(json.contains("\"pageScale\":1.5"), "{json}");
    assert_eq!(node_page_session_key("b"), "node:b");
}
