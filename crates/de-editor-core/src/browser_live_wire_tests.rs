use super::*;

fn page() -> BrowserSnapshotSpec {
    BrowserSnapshotSpec {
        url: "https://example.com".into(),
        layout_width: 390,
        layout_height: 844,
        scale: 2.0,
        mobile: true,
        scroll_x: 0.0,
        scroll_y: 0.0,
    }
}

#[test]
fn a_set_round_trips_in_camel_case() {
    let set = BrowserLiveSet {
        client: "c1".into(),
        nodes: vec![BrowserLiveEntry {
            key: "node-1".into(),
            page: page(),
            frame_width: 512,
            frame_height: 1024,
        }],
    };
    let json = serde_json::to_string(&set).unwrap();
    assert!(json.contains("\"frameWidth\":512"), "{json}");
    assert!(json.contains("\"layoutWidth\":390"), "{json}");
    assert_eq!(serde_json::from_str::<BrowserLiveSet>(&json).unwrap(), set);
}

#[test]
fn events_are_tagged_by_type() {
    let events = [
        BrowserLiveEvent::Hello {
            client: "c1".into(),
        },
        BrowserLiveEvent::Frame {
            key: "n".into(),
            seq: 7,
            width: 320,
            height: 200,
            jpeg_base64: "/9j/".into(),
        },
        BrowserLiveEvent::Blocked { key: "n".into() },
        BrowserLiveEvent::Ended {
            key: "n".into(),
            reason: BrowserLiveEndReason::NoBrowser,
            message: "no browser".into(),
        },
    ];
    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            serde_json::from_str::<BrowserLiveEvent>(&json).unwrap(),
            event
        );
    }
    let frame = serde_json::to_string(&BrowserLiveEvent::Frame {
        key: "n".into(),
        seq: 1,
        width: 1,
        height: 1,
        jpeg_base64: String::new(),
    })
    .unwrap();
    assert!(frame.starts_with("{\"type\":\"frame\""), "{frame}");
    assert!(frame.contains("\"jpegBase64\""), "{frame}");
    let ended = serde_json::to_string(&BrowserLiveEvent::Ended {
        key: "n".into(),
        reason: BrowserLiveEndReason::NoBrowser,
        message: String::new(),
    })
    .unwrap();
    assert!(ended.contains("\"reason\":\"noBrowser\""), "{ended}");
}

#[test]
fn a_reply_reads_with_missing_lists() {
    let reply: BrowserLiveSetReply =
        serde_json::from_str(r#"{"ok":false,"error":"unknown client"}"#).unwrap();
    assert!(!reply.ok && reply.live.is_empty());
    assert_eq!(reply.error.as_deref(), Some("unknown client"));
    let ok = serde_json::to_string(&BrowserLiveSetReply {
        ok: true,
        live: vec!["a".into()],
        ..Default::default()
    })
    .unwrap();
    assert!(!ok.contains("error"), "{ok}");
}

#[test]
fn a_stream_entry_keeps_the_snapshot_layout_at_the_stream_scale() {
    let node: jian_ops_schema::node::PenNode = serde_json::from_str(
        r#"{"type":"browser","id":"b","width":390,"height":844,"url":"https://example.com","deviceId":"phone"}"#,
    )
    .unwrap();
    let request = crate::browser_live::browser_render_request(&node, 0, None).unwrap();
    let tier = crate::browser_stream::stream_tier(crate::browser_stream::StreamHost::Web);
    let spec = crate::browser_stream::BrowserStreamSpec::for_request(&request, 512, tier);
    let entry = BrowserLiveEntry::for_stream("b#1".into(), &spec);
    assert_eq!(entry.key, "b#1");
    assert_eq!(entry.page.url, request.url);
    assert_eq!(
        (entry.page.layout_width, entry.page.layout_height),
        (request.layout_width, request.layout_height)
    );
    assert_eq!(entry.page.mobile, request.mobile);
    assert_eq!(entry.page.scale, spec.page_scale);
    assert_eq!((entry.page.scroll_x, entry.page.scroll_y), (0.0, 0.0));
    assert_eq!(
        (entry.frame_width, entry.frame_height),
        (spec.frame_width, spec.frame_height)
    );
}
