use super::*;

#[test]
fn the_body_is_validated_before_anything_renders() {
    let spec = parse_spec(
        r#"{"url":"example.com","layoutWidth":390,"layoutHeight":844,"scale":2,"mobile":true,"scrollY":120}"#,
    )
    .unwrap();
    assert_eq!(spec.url, "https://example.com");
    assert_eq!((spec.layout_width, spec.layout_height), (390, 844));
    assert!(spec.mobile);
    assert_eq!(spec.scroll_y, 120.0);
    for body in [
        "nope",
        r#"{"layoutWidth":1,"layoutHeight":1,"scale":1}"#,
        r#"{"url":"javascript:alert(1)","layoutWidth":1,"layoutHeight":1,"scale":1}"#,
        r#"{"url":"https://example.com","layoutWidth":0,"layoutHeight":1,"scale":1}"#,
        r#"{"url":"https://example.com","layoutWidth":1,"layoutHeight":99999,"scale":1}"#,
        r#"{"url":"https://example.com","layoutWidth":1,"layoutHeight":1,"scale":9}"#,
    ] {
        assert!(parse_spec(body).is_err(), "{body}");
    }
    let (status, reply) = snapshot_reply("nope");
    assert_eq!(status, "400 Bad Request");
    assert!(reply.contains("\"ok\":false"));
}

#[test]
fn the_wire_shape_is_the_core_spec() {
    let request = de_editor_core::browser_live::BrowserSnapshotSpec {
        url: "https://example.com".into(),
        layout_width: 390,
        layout_height: 844,
        scale: 2.0,
        mobile: true,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let body = serde_json::to_string(&request).unwrap();
    assert!(body.contains("\"layoutWidth\":390") && body.contains("\"scrollY\":40"));
    let spec = parse_spec(&body).unwrap();
    assert_eq!(spec, render_spec(&request));
}

#[test]
fn a_blocked_address_never_reaches_the_browser() {
    crate::browser_url_policy::screen_for_daemon("0.0.0.0".parse().unwrap());
    let (status, reply) = snapshot_reply(
        r#"{"url":"http://169.254.169.254/","layoutWidth":100,"layoutHeight":100,"scale":1}"#,
    );
    assert_eq!(status, "403 Forbidden");
    assert!(reply.contains("\"reason\":\"blocked\""));
}
