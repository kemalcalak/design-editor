//! Website-style browsing (#153): link navigation, back / forward
//! history, the address, section links, and clickable hover.

#![cfg(test)]

use super::{test_measure, PreviewCursor, PreviewSession};
use jian_core::action::services::Router;
use jian_core::gesture::pointer::{PointerKind, PointerPhase};

/// Three routed desktop pages. Header links use `replace` (the auto-wire
/// default), so the router stack never grows; `features-link` scrolls to a
/// section of the same page.
const SITE_JSON: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "site",
    "app": { "name": "site", "version": "1", "id": "site" },
    "children": [
        { "type": "frame", "id": "home", "name": "Home", "screen": "/",
          "x": 0, "y": 0, "width": 1280, "height": 2000,
          "children": [
              { "type": "frame", "id": "to-pricing", "x": 20, "y": 10,
                "width": 120, "height": 40,
                "fill": [{ "type": "solid", "color": "#336699" }],
                "events": { "onTap": [ { "replace": "\"/pricing\"" } ] },
                "children": [
                    { "type": "text", "id": "to-pricing-label", "x": 10, "y": 10,
                      "content": "Pricing" }
                ] },
              { "type": "text", "id": "features-link", "x": 200, "y": 10,
                "width": 100, "height": 30, "content": "Features",
                "events": { "onTap": [ { "scroll_to": { "target": "features" } } ] } },
              { "type": "text", "id": "plain", "x": 400, "y": 10,
                "width": 100, "height": 30, "content": "Not a link" },
              { "type": "frame", "id": "features", "name": "Features",
                "x": 0, "y": 1200, "width": 1280, "height": 400 }
          ] },
        { "type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
          "x": 1400, "y": 0, "width": 1280, "height": 900,
          "children": [
              { "type": "frame", "id": "to-about", "x": 20, "y": 10,
                "width": 120, "height": 40,
                "events": { "onTap": [ { "replace": "\"/about\"" } ] } }
          ] },
        { "type": "frame", "id": "about", "name": "About", "screen": "/about",
          "x": 2800, "y": 0, "width": 1280, "height": 900 }
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

fn centre(session: &PreviewSession, id: &str) -> (f32, f32) {
    let (x, y, w, h) = session.node_rect(id).expect("node laid out");
    (x + w / 2.0, y + h / 2.0)
}

/// Tap `id` after any running screen transition has finished.
fn click(session: &mut PreviewSession, id: &str, now: &mut u64) {
    *now += 5_000;
    session.set_now_ms(*now);
    let (x, y) = centre(session, id);
    session.dispatch_tap(x, y);
}

fn settle(session: &mut PreviewSession, now: &mut u64) -> bool {
    *now += 5_000;
    session.reconcile(*now).switched
}

fn hover(session: &mut PreviewSession, x: f32, y: f32) {
    session.dispatch_pointer_for_id_at(1, PointerKind::Mouse, x, y, PointerPhase::Hover, 0);
}

#[test]
fn link_click_opens_the_target_page_and_records_history() {
    let mut session = enter();
    let mut now = 0;
    assert!(!session.can_go_back());
    click(&mut session, "to-pricing", &mut now);
    assert!(settle(&mut session, &mut now), "the link switches page");
    assert_eq!(session.current_path_for_test(), "/pricing");
    assert!(session.can_go_back());
    assert!(!session.can_go_forward());
    assert_eq!(
        session.router_for_test().current().stack.len(),
        1,
        "a replace link keeps the router stack flat; history still goes back"
    );
}

#[test]
fn back_and_forward_walk_the_history() {
    let mut session = enter();
    let mut now = 0;
    click(&mut session, "to-pricing", &mut now);
    settle(&mut session, &mut now);
    click(&mut session, "to-about", &mut now);
    settle(&mut session, &mut now);
    assert_eq!(session.current_path_for_test(), "/about");

    assert_eq!(session.go_back(), crate::InvalidationKind::Navigation);
    assert!(settle(&mut session, &mut now));
    assert_eq!(session.current_path_for_test(), "/pricing");
    session.go_back();
    settle(&mut session, &mut now);
    assert_eq!(session.current_path_for_test(), "/");
    assert!(!session.can_go_back());
    assert_eq!(
        session.go_back(),
        crate::InvalidationKind::None,
        "back at the oldest entry is a no-op"
    );

    session.go_forward();
    settle(&mut session, &mut now);
    assert_eq!(session.current_path_for_test(), "/pricing");
    assert!(session.can_go_forward());

    // A new link from the middle of the history drops the forward entries.
    click(&mut session, "to-about", &mut now);
    settle(&mut session, &mut now);
    session.go_back();
    settle(&mut session, &mut now);
    click(&mut session, "to-about", &mut now);
    settle(&mut session, &mut now);
    assert!(!session.can_go_forward());
}

#[test]
fn back_pops_a_pushed_screen_so_the_router_stack_stays_honest() {
    let mut session = enter();
    let mut now = 0;
    session.router_for_test().push("/about");
    settle(&mut session, &mut now);
    assert_eq!(session.router_for_test().current().stack.len(), 2);
    session.go_back();
    settle(&mut session, &mut now);
    assert_eq!(session.current_path_for_test(), "/");
    assert_eq!(session.router_for_test().current().stack.len(), 1);
    session.go_forward();
    settle(&mut session, &mut now);
    assert_eq!(session.current_path_for_test(), "/about");
    assert_eq!(
        session.router_for_test().current().stack.len(),
        2,
        "forward re-pushes what was pushed"
    );
}

#[test]
fn address_shows_the_mounted_page_title_and_route() {
    let mut session = enter();
    let mut now = 0;
    let address = session.browse_address().expect("app mode has an address");
    assert_eq!(address.route, "/");
    assert_eq!(address.title, "Home");
    click(&mut session, "to-pricing", &mut now);
    settle(&mut session, &mut now);
    let address = session.browse_address().unwrap();
    assert_eq!(
        (address.route.as_str(), address.title.as_str()),
        ("/pricing", "Pricing")
    );
}

#[test]
fn section_link_hands_the_host_a_page_scroll_request() {
    let mut session = enter();
    let mut now = 0;
    assert!(session.take_page_scroll_request().is_none());
    click(&mut session, "features-link", &mut now);
    let request = session
        .take_page_scroll_request()
        .expect("scroll_to the page's own section becomes a host request");
    assert!(
        (request.offset - 1200.0).abs() < 0.5,
        "offset puts the section top at the page top, got {}",
        request.offset
    );
    assert!(request.page_rect.size.y >= 2000.0 - 0.5);
    assert!(
        session.take_page_scroll_request().is_none(),
        "a request is handed out once"
    );
}

#[test]
fn hovering_a_link_child_resolves_the_link_and_a_pointer_cursor() {
    let mut session = enter();
    let (x, y) = centre(&session, "to-pricing-label");
    hover(&mut session, x, y);
    assert_eq!(session.hovered_clickable().as_deref(), Some("to-pricing"));
    assert_eq!(session.preview_cursor(), PreviewCursor::Pointer);

    let (x, y) = centre(&session, "plain");
    hover(&mut session, x, y);
    assert_eq!(session.hovered_clickable(), None);
    assert_eq!(session.preview_cursor(), PreviewCursor::Default);
}

#[test]
fn hovered_link_paints_a_hover_state_without_touching_the_document() {
    let mut session = enter();
    let find = |session: &PreviewSession, id: &str| {
        session
            .preview_scene_for_test()
            .active_page()
            .and_then(|page| page.find(id).cloned())
            .expect("node in scene")
    };
    let filled_before = find(&session, "to-pricing").fill;
    let text_before = find(&session, "features-link").opacity;

    let (x, y) = centre(&session, "to-pricing");
    hover(&mut session, x, y);
    assert_ne!(
        find(&session, "to-pricing").fill,
        filled_before,
        "a filled link tints on hover"
    );

    let (x, y) = centre(&session, "features-link");
    hover(&mut session, x, y);
    assert!(
        find(&session, "features-link").opacity < text_before,
        "a fill-less text link dims on hover"
    );
    assert_eq!(
        find(&session, "to-pricing").fill,
        filled_before,
        "the previous link's hover state is gone"
    );
}

/// An unmarked two-page website: preview entry auto-wires its header
/// link (`gl-host` links the orchestrator pass), and a click browses.
#[cfg(feature = "gl-host")]
#[test]
fn unmarked_website_header_link_browses_after_auto_wire() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":1280,"height":1600,
         "layout":"vertical","children":[
            {"type":"frame","id":"header","name":"Header","layout":"horizontal",
             "width":1280,"height":64,"children":[
                {"type":"text","id":"to-pricing","content":"Pricing"}
            ]}
         ]},
        {"type":"frame","id":"pricing","name":"Pricing","x":1400,"width":1280,"height":900,
         "layout":"vertical","children":[]}
    ]}"#;
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(json).unwrap();
    let mut session = PreviewSession::enter(
        &doc,
        (1400.0, 900.0),
        &Default::default(),
        0,
        false,
        false,
        test_measure(),
        0,
    )
    .expect("enter");
    assert!(session.is_app_mode(), "two desktop pages auto-route");
    let mut now = 0;
    click(&mut session, "to-pricing", &mut now);
    assert!(settle(&mut session, &mut now));
    assert_eq!(session.current_path_for_test(), "/pricing");
    assert!(session.can_go_back());
}
