//! pen.dev `browser` nodes import as the canonical browser node (#20)
//! instead of being dropped: the frame, page settings, corner radius,
//! stroke and effects survive.

use jian_ops_schema::node::container::CornerRadius;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::{PenEffect, StrokeThickness};
use jian_ops_schema::PenDocument;
use serde_json::{json, Value};

use crate::payload::load_canonical_with_assets;
use crate::pen_dev::{no_assets, PenDevReport};

fn load(children: Value) -> (PenDocument, PenDevReport) {
    let src = json!({ "version": "2.21", "children": children }).to_string();
    let loaded = load_canonical_with_assets(&src, &no_assets).expect("pen.dev document loads");
    let report = loaded
        .compatibility
        .pen_dev
        .clone()
        .expect("converted as pen.dev");
    (loaded.loaded.value, report)
}

#[test]
fn a_browser_node_keeps_its_frame_page_and_style() {
    let (doc, report) = load(json!([{
        "type": "browser", "id": "b", "name": "Landing", "x": 40, "y": 60,
        "width": 390, "height": 844, "url": "https://example.com/pricing",
        "deviceId": "phone", "zoom": 0.75, "scrollX": 0, "scrollY": 320,
        "cornerRadius": 24, "stroke": "#1F2937", "strokeWidth": 2,
        "effect": { "type": "shadow", "shadowType": "outer", "color": "#00000040",
                    "offset": { "x": 0, "y": 12 }, "blur": 32 }
    }]));
    assert!(!report.has_losses(), "{}", report.summary());
    let PenNode::Browser(browser) = &doc.children[0] else {
        panic!("browser imports as a browser node: {:?}", doc.children[0]);
    };
    assert_eq!(browser.base.name.as_deref(), Some("Landing"));
    assert_eq!((browser.base.x, browser.base.y), (Some(40.0), Some(60.0)));
    assert_eq!(browser.width, Some(SizingBehavior::Number(390.0)));
    assert_eq!(browser.height, Some(SizingBehavior::Number(844.0)));
    assert_eq!(browser.url.as_deref(), Some("https://example.com/pricing"));
    assert_eq!(browser.device_id.as_deref(), Some("phone"));
    assert_eq!(browser.zoom, Some(0.75));
    assert_eq!(
        (browser.scroll_x, browser.scroll_y),
        (Some(0.0), Some(320.0))
    );
    assert_eq!(browser.corner_radius, Some(CornerRadius::Uniform(24.0)));
    let stroke = browser.stroke.as_ref().expect("stroke kept");
    assert!(matches!(stroke.thickness, StrokeThickness::Uniform(w) if w == 2.0));
    assert!(matches!(
        browser.effects.as_deref(),
        Some([PenEffect::Shadow(_)])
    ));
}

#[test]
fn a_browser_inside_a_frame_is_kept_with_its_siblings() {
    let (doc, report) = load(json!([{ "type": "frame", "id": "f", "layout": "none",
    "children": [
        { "type": "browser", "id": "b", "url": "https://example.com" },
        { "type": "text", "id": "t", "content": "kept" }
    ]}]));
    assert_eq!(report.unsupported.get("browser node"), None);
    let PenNode::Frame(frame) = &doc.children[0] else {
        panic!()
    };
    let kids = frame.children.as_deref().unwrap();
    assert_eq!(kids.len(), 2);
    assert!(kids.iter().any(|kid| matches!(kid, PenNode::Browser(_))));
}
