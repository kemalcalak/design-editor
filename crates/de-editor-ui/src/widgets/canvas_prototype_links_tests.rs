use super::*;
use crate::layout_scene::{NodeKind, SceneNode};
use crate::widgets::test_capture_backend::CaptureBackend;
use de_editor_core::NodeId;

const DOC: &str = r#"{
  "version": "1.0", "children": [],
  "pages": [
    {"id": "site", "name": "Site", "children": [
      {"type": "frame", "id": "home", "name": "Home", "screen": "/", "width": 400, "height": 800,
       "children": [
        {"type": "ref", "id": "cta", "ref": "btn"},
        {"type": "frame", "id": "go", "name": "Go", "width": 80, "height": 40,
         "events": {"onTap": [{"push": "\"/pricing\""}], "onHoverEnter": [{"pop": null}]}},
        {"type": "frame", "id": "dead", "name": "Dead", "width": 80, "height": 40,
         "events": {"onTap": [{"push": "\"/old\""}]}},
        {"type": "frame", "id": "out", "name": "Out", "width": 80, "height": 40,
         "events": {"onLongPress": [{"open_url": {"url": "\"https://www.example.com/a\""}}]}}
       ]},
      {"type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
       "x": 600, "width": 400, "height": 800}
    ]},
    {"id": "kit", "name": "Components", "children": [
      {"type": "frame", "id": "btn", "name": "Button", "reusable": true, "width": 160, "height": 48,
       "events": {"onTap": [{"replace": "\"/pricing\""}]}}
    ]}
  ]
}"#;

fn state() -> EditorState {
    EditorState::from_document(serde_json::from_str(DOC).expect("fixture"))
}

#[test]
fn the_page_links_expand_instances_and_sort_out_kinds() {
    let s = state();
    let links = page_links(&s);
    let find = |anchor: &str, trigger: InteractionTrigger| {
        links
            .iter()
            .find(|l| l.anchor == anchor && l.trigger == trigger)
            .unwrap_or_else(|| panic!("{anchor} {trigger:?} in {links:#?}"))
            .kind
            .clone()
    };
    let arrow = CanvasLinkKind::Arrow {
        target: "pricing".into(),
    };
    assert_eq!(find("go", InteractionTrigger::Tap), arrow);
    assert_eq!(
        find("cta", InteractionTrigger::Tap),
        arrow,
        "the instance links"
    );
    assert_eq!(find("go", InteractionTrigger::Hover), CanvasLinkKind::Back);
    assert_eq!(
        find("dead", InteractionTrigger::Tap),
        CanvasLinkKind::Broken
    );
    assert_eq!(
        find("out", InteractionTrigger::LongPress),
        CanvasLinkKind::OpenUrl {
            host: "example.com".into()
        }
    );
    // The master is on another page: not part of this page's links.
    assert!(links.iter().all(|l| l.anchor != "btn"));
}

#[test]
fn links_show_for_the_selection_or_with_the_view_toggle() {
    let mut s = state();
    assert!(visible_links(&s).is_empty(), "nothing selected, toggle off");
    s.set_single_selection(NodeId::new("go"));
    let shown = visible_links(&s);
    assert!(!shown.is_empty() && shown.iter().all(|l| l.anchor == "go"));
    s.set_single_selection(NodeId::new("home"));
    assert_eq!(
        visible_links(&s).len(),
        page_links(&s).len(),
        "a screen shows its links"
    );
    s.clear_selection();
    s.editor_ui.prototype.show_links = true;
    assert_eq!(visible_links(&s).len(), page_links(&s).len());
}

fn node(id: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, NodeKind::Frame);
    n.bounds = Rect::xywh(x, y, w, h);
    n
}

#[test]
fn paint_draws_labels_badges_and_red_for_broken_links() {
    let s = state();
    let page = ScenePage {
        id: "site".into(),
        name: "Site".into(),
        children: vec![
            node("go", 10.0, 10.0, 80.0, 40.0),
            node("dead", 10.0, 80.0, 80.0, 40.0),
            node("pricing", 600.0, 0.0, 400.0, 800.0),
        ],
    };
    let viewport = de_editor_core::Viewport {
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.0,
    };
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let links = page_links(&s);
    let canvas = Rect::xywh(0.0, 0.0, 1200.0, 900.0);
    paint(
        &mut cx,
        &page,
        &links,
        canvas,
        &viewport,
        de_editor_core::Locale::EnUs,
    );
    let texts: Vec<&str> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"Tap"), "{texts:?}");
    assert!(texts.contains(&"Hover · Back"), "{texts:?}");
    assert!(texts.contains(&"Tap · Broken link"), "{texts:?}");
    assert!(backend.round_fills.iter().any(|(_, _, c)| *c == BROKEN));
    // Panned far away: everything is culled.
    let away = de_editor_core::Viewport {
        pan_x: -10_000.0,
        pan_y: 0.0,
        zoom: 1.0,
    };
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint(
        &mut cx,
        &page,
        &links,
        canvas,
        &away,
        de_editor_core::Locale::EnUs,
    );
    assert!(backend.texts.is_empty());
}

const OVERLAY_DOC: &str = r#"{
  "version": "1.0", "children": [],
  "pages": [
    {"id": "site", "name": "Site", "children": [
      {"type": "frame", "id": "home", "name": "Home", "width": 400, "height": 800,
       "children": [
        {"type": "frame", "id": "open", "name": "Open", "width": 80, "height": 40,
         "events": {"onTap": [{"open_overlay": {"target": "menu"}}]}},
        {"type": "frame", "id": "far", "name": "Far", "width": 80, "height": 40,
         "events": {"onTap": [{"swap_overlay": {"target": "sheet"}}]}},
        {"type": "frame", "id": "shut", "name": "Shut", "width": 80, "height": 40,
         "events": {"onTap": [{"close_overlay": null}]}}
       ]},
      {"type": "frame", "id": "menu", "name": "Menu", "x": 600, "width": 200, "height": 300}
    ]},
    {"id": "more", "name": "Overlays", "children": [
      {"type": "frame", "id": "sheet", "name": "Sheet", "width": 400, "height": 300}
    ]}
  ]
}"#;

#[test]
fn overlay_links_draw_dashed_or_badge_a_frame_elsewhere() {
    let s = EditorState::from_document(serde_json::from_str(OVERLAY_DOC).expect("fixture"));
    let links = page_links(&s);
    let kind = |anchor: &str| {
        links
            .iter()
            .find(|l| l.anchor == anchor)
            .map(|l| l.kind.clone())
            .unwrap_or_else(|| panic!("{anchor} in {links:#?}"))
    };
    assert_eq!(
        kind("open"),
        CanvasLinkKind::OverlayArrow {
            target: "menu".into()
        }
    );
    assert_eq!(
        kind("far"),
        CanvasLinkKind::OverlayElsewhere {
            name: "Sheet".into()
        }
    );
    assert_eq!(kind("shut"), CanvasLinkKind::CloseOverlay);

    let page = ScenePage {
        id: "site".into(),
        name: "Site".into(),
        children: vec![
            node("open", 10.0, 10.0, 80.0, 40.0),
            node("menu", 600.0, 0.0, 200.0, 300.0),
        ],
    };
    let viewport = de_editor_core::Viewport {
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.0,
    };
    let only_open: Vec<CanvasLink> = links
        .iter()
        .filter(|l| l.anchor == "open")
        .cloned()
        .collect();
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    paint(
        &mut cx,
        &page,
        &only_open,
        Rect::xywh(0.0, 0.0, 1200.0, 900.0),
        &viewport,
        de_editor_core::Locale::EnUs,
    );
    assert_eq!(
        backend.lines,
        CURVE_STEPS / 2,
        "every other segment: dashed"
    );
    let texts: Vec<&str> = backend.texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"Tap · Overlay"), "{texts:?}");
}

#[test]
fn url_hosts_drop_the_scheme_path_and_www() {
    assert_eq!(url_host("https://www.example.com/a?b"), "example.com");
    assert_eq!(url_host("example.org"), "example.org");
}
