use jian_ops_schema::node::PenNode;

use super::*;

fn browser(json: &str) -> PenNode {
    serde_json::from_str(json).expect("browser node")
}

#[test]
fn a_phone_preset_lays_out_at_its_width_and_the_box_aspect() {
    let node = browser(
        r#"{"type":"browser","id":"b","width":390,"height":844,"url":"https://example.com","deviceId":"phone"}"#,
    );
    let request = browser_render_request(&node, 0).unwrap();
    assert_eq!((request.layout_width, request.layout_height), (390, 844));
    assert!(request.mobile);
    assert_eq!(request.scale, 2.0);
    assert_eq!(request.node_id, "b");
}

#[test]
fn zoom_shrinks_the_layout_and_big_boxes_cap_the_snapshot() {
    let node = browser(
        r#"{"type":"browser","id":"b","width":1440,"height":900,"url":"https://example.com","zoom":2}"#,
    );
    let request = browser_render_request(&node, 0).unwrap();
    assert_eq!((request.layout_width, request.layout_height), (720, 450));
    assert!(!request.mobile);
    let out_w = request.layout_width as f64 * request.scale;
    assert!(out_w <= MAX_SNAPSHOT_EDGE + 1.0, "out_w = {out_w}");
}

#[test]
fn the_key_changes_with_what_the_picture_shows_and_with_a_reload() {
    let base =
        r#"{"type":"browser","id":"b","width":800,"height":600,"url":"https://example.com"}"#;
    let key = |json: &str, reload| browser_render_request(&browser(json), reload).unwrap().key;
    let k = key(base, 0);
    assert_eq!(k, key(base, 0));
    assert_ne!(k, key(base, 1));
    assert_ne!(k, key(&base.replace("example.com", "example.org"), 0));
    assert_ne!(k, key(&base.replace("600", "700"), 0));
    assert_ne!(k, key(&base.replace("}", r#","scrollY":40}"#), 0));
}

#[test]
fn blank_addresses_and_other_kinds_need_no_snapshot() {
    assert!(browser_render_request(
        &browser(r#"{"type":"browser","id":"b","width":10,"height":10}"#),
        0
    )
    .is_none());
    let rect: PenNode = serde_json::from_str(r#"{"type":"rectangle","id":"r"}"#).unwrap();
    assert!(browser_render_request(&rect, 0).is_none());
}

#[test]
fn live_state_holds_reloads_and_statuses() {
    let mut live = BrowserLiveState::default();
    assert!(live.pages_allowed());
    live.hold_pages();
    assert!(!live.pages_allowed());
    live.allow_pages();
    live.reload("b");
    live.reload("b");
    assert_eq!(live.reload_count("b"), 2);
    live.set_status("b", BrowserPageStatus::Failed);
    assert_eq!(live.status("b"), Some(BrowserPageStatus::Failed));
    live.clear_nodes();
    assert_eq!((live.reload_count("b"), live.status("b")), (0, None));
}
