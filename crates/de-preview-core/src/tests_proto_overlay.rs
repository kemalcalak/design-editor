//! Prototype overlays in Preview (#372): stacking, outside clicks, Back,
//! Esc, swap, overlays on instances, and navigating out of one.

#![cfg(test)]

use super::{test_measure, CaptureStep, PreviewSession};

const DOC: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "ovl",
    "app": { "name": "ovl", "version": "1", "id": "ovl" },
    "children": [
        { "type": "frame", "id": "home", "name": "Home", "screen": "/",
          "x": 0, "y": 0, "width": 400, "height": 800,
          "children": [
              { "type": "frame", "id": "open-menu", "x": 20, "y": 20, "width": 120, "height": 40,
                "events": { "onTap": [ { "open_overlay": { "target": "menu",
                    "background": "#00000066" } } ] } },
              { "type": "frame", "id": "open-modal", "x": 20, "y": 80, "width": 120, "height": 40,
                "events": { "onTap": [ { "open_overlay": { "target": "modal",
                    "closeOnOutsideClick": false } } ] } },
              { "type": "frame", "id": "open-card", "x": 20, "y": 140, "width": 120, "height": 40,
                "events": { "onTap": [ { "open_overlay": { "target": "card-1",
                    "position": "bottom_center" } } ] } },
              { "type": "frame", "id": "open-drop", "x": 200, "y": 20, "width": 100, "height": 30,
                "events": { "onTap": [ { "open_overlay": { "target": "modal",
                    "position": "manual", "offset": { "x": 0, "y": 4 } } } ] } },
              { "type": "frame", "id": "corner", "x": 0, "y": 760, "width": 40, "height": 40,
                "events": { "onTap": [ { "replace": "\"/pricing\"" } ] } }
          ] },
        { "type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
          "x": 500, "y": 0, "width": 400, "height": 800 },
        { "type": "frame", "id": "menu", "name": "Menu", "x": 1000, "y": 0,
          "width": 200, "height": 300,
          "children": [
              { "type": "frame", "id": "menu-close", "x": 0, "y": 0, "width": 200, "height": 40,
                "events": { "onTap": [ { "close_overlay": null } ] } },
              { "type": "frame", "id": "menu-swap", "x": 0, "y": 50, "width": 200, "height": 40,
                "events": { "onTap": [ { "swap_overlay": { "target": "modal" } } ] } },
              { "type": "frame", "id": "menu-next", "x": 0, "y": 100, "width": 200, "height": 40,
                "events": { "onTap": [ { "open_overlay": { "target": "sheet" } } ] } },
              { "type": "frame", "id": "menu-back", "x": 0, "y": 150, "width": 200, "height": 40,
                "events": { "onTap": [ { "pop": null } ] } },
              { "type": "frame", "id": "menu-nav", "x": 0, "y": 200, "width": 200, "height": 40,
                "events": { "onTap": [ { "replace": "\"/pricing\"" } ] } }
          ] },
        { "type": "frame", "id": "modal", "name": "Modal", "x": 1300, "y": 0,
          "width": 300, "height": 200 },
        { "type": "frame", "id": "sheet", "name": "Sheet", "x": 1700, "y": 0,
          "width": 400, "height": 2000 },
        { "type": "frame", "id": "card", "name": "Card", "reusable": true, "x": 2200, "y": 0,
          "width": 160, "height": 100,
          "children": [
              { "type": "frame", "id": "card-close", "x": 0, "y": 0, "width": 160, "height": 40,
                "events": { "onTap": [ { "close_overlay": null } ] } }
          ] },
        { "type": "ref", "id": "card-1", "ref": "card", "x": 2400, "y": 0 }
    ]
}"##;

fn enter() -> PreviewSession {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(DOC).unwrap();
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
    .expect("enter preview")
}

fn tap(session: &mut PreviewSession, id: &str) {
    session
        .run_capture_script(&[CaptureStep::Tap(id.to_owned())], 0)
        .unwrap_or_else(|e| panic!("tap {id}: {e}"));
}

fn tap_at(session: &mut PreviewSession, x: f32, y: f32) {
    use jian_core::gesture::pointer::PointerPhase;
    let now = session.now_ms();
    session.dispatch_pointer_phase_at(x, y, PointerPhase::Down, now);
    session.dispatch_pointer_phase_at(x, y, PointerPhase::Up, now + 16);
    session.advance_to(now + 400);
}

#[test]
fn an_overlay_opens_over_the_screen_with_its_dim_centred() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    assert_eq!(s.open_overlay_targets(), ["menu"]);
    let scene = s.painted_scene();
    let page = scene.active_page().expect("page");
    let menu = page.find("menu").expect("the overlay is painted");
    // Centred in the 400 × 800 screen.
    assert_eq!((menu.bounds.origin.x, menu.bounds.origin.y), (100.0, 250.0));
    let dim = page.find("menu#overlay-dim-0").expect("the dim");
    assert!(dim.fill.is_some_and(|c| (c.a - 0.4).abs() < 0.01));
    assert_eq!(s.current_path().as_deref(), Some("/"));
}

#[test]
fn overlays_stack_and_the_same_one_never_stacks_twice() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-next");
    assert_eq!(s.open_overlay_targets(), ["menu", "sheet"]);
    let spec = de_editor_core::overlay_action::OverlaySpec::new("sheet");
    assert!(!s.open_overlay(spec, None));
    assert_eq!(s.open_overlay_targets(), ["menu", "sheet"]);
}

#[test]
fn an_outside_click_closes_when_allowed_and_is_blocked_otherwise() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap_at(&mut s, 5.0, 5.0);
    assert!(!s.has_open_overlays(), "outside click closes the menu");

    tap(&mut s, "open-modal");
    assert_eq!(s.open_overlay_targets(), ["modal"]);
    // The corner link under the modal would navigate: it is blocked.
    tap_at(&mut s, 20.0, 780.0);
    s.advance_to(s.now_ms() + 400);
    assert_eq!(s.open_overlay_targets(), ["modal"]);
    assert_eq!(s.current_path().as_deref(), Some("/"));
}

#[test]
fn back_and_escape_close_the_top_overlay_and_forward_reopens_it() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-next");
    assert!(s.can_go_back());
    s.go_back();
    assert_eq!(s.open_overlay_targets(), ["menu"]);
    assert!(s.can_go_forward());
    s.go_forward();
    assert_eq!(s.open_overlay_targets(), ["menu", "sheet"]);
    assert!(s.close_top_overlay(), "Esc");
    assert_eq!(s.open_overlay_targets(), ["menu"]);
    // An authored Back (`pop`) inside the overlay closes it, not the screen.
    tap(&mut s, "menu-back");
    assert!(!s.has_open_overlays());
    assert_eq!(s.current_path().as_deref(), Some("/"));
}

#[test]
fn swap_replaces_the_top_overlay_and_close_closes_it() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-swap");
    assert_eq!(s.open_overlay_targets(), ["modal"]);
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-close");
    assert!(!s.has_open_overlays());
}

#[test]
fn an_instance_overlay_shows_and_its_master_link_closes_it() {
    let mut s = enter();
    tap(&mut s, "open-card");
    assert_eq!(s.open_overlay_targets(), ["card-1"]);
    let scene = s.painted_scene();
    let card = scene
        .active_page()
        .and_then(|p| p.find("card-1"))
        .expect("card");
    // Bottom centre of the 400 × 800 screen.
    assert_eq!((card.bounds.origin.x, card.bounds.origin.y), (120.0, 700.0));
    tap(&mut s, "card-1/card-close");
    assert!(!s.has_open_overlays());
}

#[test]
fn a_manual_overlay_sits_under_its_trigger() {
    let mut s = enter();
    tap(&mut s, "open-drop");
    let scene = s.painted_scene();
    let modal = scene
        .active_page()
        .and_then(|p| p.find("modal"))
        .expect("modal");
    assert_eq!(
        (modal.bounds.origin.x, modal.bounds.origin.y),
        (200.0, 54.0)
    );
}

#[test]
fn a_tall_overlay_scrolls_inside_itself() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-next");
    let scene = s.painted_scene();
    let clip = scene
        .active_page()
        .and_then(|p| p.find("sheet#overlay-clip"))
        .expect("a clipping box");
    assert_eq!(clip.bounds.size.y, 800.0);
    assert!(s.dispatch_wheel(200.0, 400.0, 0.0, -300.0));
    let scene = s.painted_scene();
    let sheet = scene
        .active_page()
        .and_then(|p| p.find("sheet"))
        .expect("sheet");
    assert_eq!(sheet.bounds.origin.y, -300.0);
}

#[test]
fn navigating_from_inside_an_overlay_closes_every_overlay() {
    let mut s = enter();
    tap(&mut s, "open-menu");
    tap(&mut s, "menu-nav");
    assert_eq!(s.current_path().as_deref(), Some("/pricing"));
    assert!(!s.has_open_overlays());
}
