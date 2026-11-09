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
