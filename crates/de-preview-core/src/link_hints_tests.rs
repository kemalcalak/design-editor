//! Link hints, hotspots and instance taps in Preview (#371).

use super::*;
use crate::{test_measure, CaptureStep};
use jian_core::gesture::pointer::{PointerKind, PointerPhase};

/// Two routed screens. The home screen holds a link, a back button, a URL
/// button, a section link, plain text and an instance of a button master
/// whose label navigates (the master lives on the same page, outside both
/// screens).
const SITE_JSON: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "site",
    "app": { "name": "site", "version": "1", "id": "site" },
    "children": [
        { "type": "frame", "id": "home", "name": "Home", "screen": "/",
          "x": 0, "y": 0, "width": 1280, "height": 2000,
          "children": [
              { "type": "frame", "id": "to-pricing", "x": 20, "y": 10,
                "width": 120, "height": 40,
                "events": { "onTap": [ { "replace": "\"/pricing\"" } ] } },
              { "type": "frame", "id": "go-back", "x": 160, "y": 10,
                "width": 60, "height": 40, "events": { "onTap": [ { "pop": null } ] } },
              { "type": "frame", "id": "docs", "x": 240, "y": 10,
                "width": 60, "height": 40,
                "events": { "onTap": [ { "open_url": { "url": "\"https://www.example.com/docs\"" } } ] } },
              { "type": "text", "id": "features-link", "x": 320, "y": 10,
                "width": 100, "height": 30, "content": "Features",
                "events": { "onTap": [ { "scroll_to": { "target": "features" } } ] } },
              { "type": "text", "id": "plain", "x": 440, "y": 10,
                "width": 100, "height": 30, "content": "Not a link" },
              { "type": "ref", "id": "buy", "ref": "btn", "x": 20, "y": 100 },
              { "type": "frame", "id": "features", "name": "Features",
                "x": 0, "y": 1200, "width": 1280, "height": 400 }
          ] },
        { "type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
          "x": 1400, "y": 0, "width": 1280, "height": 900 },
        { "type": "frame", "id": "btn", "name": "Button", "reusable": true,
          "x": 0, "y": 2200, "width": 160, "height": 48,
          "children": [
              { "type": "frame", "id": "btn-label", "name": "Buy now", "x": 0, "y": 0,
                "width": 160, "height": 48,
                "events": { "onTap": [ { "push": "\"/pricing\"" } ] } }
          ] }
    ]
}"##;

fn enter() -> PreviewSession {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(SITE_JSON).unwrap();
    PreviewSession::enter(
        &doc,
        (1400.0, 900.0),
        &Default::default(),
        0,
        false,
        false,
        test_measure(),
        0,
    )
    .expect("enter site preview")
}

fn hover_hint(session: &mut PreviewSession, id: &str) -> Option<PreviewLinkHint> {
    let (x, y, w, h) = session.node_rect(id).expect("laid out");
    session.dispatch_pointer_for_id_at(
        1,
        PointerKind::Mouse,
        x + w / 2.0,
        y + h / 2.0,
        PointerPhase::Hover,
        0,
    );
    session.hovered_link_hint()
}

#[test]
fn hovering_a_clickable_says_where_it_goes() {
    let mut s = enter();
    assert_eq!(
        hover_hint(&mut s, "to-pricing"),
        Some(PreviewLinkHint::Navigate {
            name: "Pricing".into()
        })
    );
    assert_eq!(hover_hint(&mut s, "go-back"), Some(PreviewLinkHint::Back));
    assert_eq!(
        hover_hint(&mut s, "docs"),
        Some(PreviewLinkHint::OpenUrl {
            host: "example.com".into()
        })
    );
    assert_eq!(
        hover_hint(&mut s, "features-link"),
        Some(PreviewLinkHint::ScrollTo {
            name: "Features".into()
        })
    );
    assert_eq!(hover_hint(&mut s, "plain"), None);
}

#[test]
fn the_hint_text_reads_like_a_link() {
    let locale = de_editor_core::Locale::EnUs;
    let text = |hint| crate::browse_chrome::link_hint_text(&hint, locale);
    assert_eq!(
        text(PreviewLinkHint::Navigate {
            name: "Pricing".into()
        }),
        "→ Pricing"
    );
    assert_eq!(text(PreviewLinkHint::Back), "Back");
    assert_eq!(
        text(PreviewLinkHint::OpenUrl {
            host: "example.com".into()
        }),
        "Opens example.com"
    );
}

#[test]
fn alt_shows_every_hotspot_and_they_never_take_a_click() {
    let mut s = enter();
    let ids = |s: &PreviewSession| {
        let mut out = Vec::new();
        fn walk(nodes: &[SceneNode], out: &mut Vec<String>) {
            for node in nodes {
                out.push(node.id.clone());
                walk(&node.children, out);
            }
        }
        walk(
            &s.painted_scene().active_page().expect("page").children,
            &mut out,
        );
        out
    };
    assert!(!ids(&s).iter().any(|id| id.ends_with(HOTSPOT_SUFFIX)));
    assert!(s.set_show_hotspots(true));
    assert!(!s.set_show_hotspots(true), "no change, no repaint");
    let shown = ids(&s);
    for id in ["to-pricing", "go-back", "features-link", "buy__btn-label"] {
        assert!(
            shown.contains(&format!("{id}{HOTSPOT_SUFFIX}")),
            "{id} in {shown:?}"
        );
    }
    assert!(!shown.contains(&format!("plain{HOTSPOT_SUFFIX}")));
    // A tap still lands on the link under its hotspot.
    s.run_capture_step(&CaptureStep::Tap("to-pricing".into()))
        .expect("tap");
    assert_eq!(s.current_path().as_deref(), Some("/pricing"));
}

#[test]
fn a_tap_on_an_instance_child_navigates() {
    for target in ["buy__btn-label", "buy/btn-label", "Buy now"] {
        let mut s = enter();
        s.run_capture_step(&CaptureStep::Tap(target.into()))
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        assert_eq!(s.current_path().as_deref(), Some("/pricing"), "{target}");
    }
}
