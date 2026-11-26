//! A three-page prototype the interaction tests share (#371): a site
//! page with screens, links, an instance and a broken link; an app page
//! whose link aims at the site page; a components page holding the
//! button master the site's instance uses.

use crate::EditorState;
use jian_ops_schema::PenDocument;

pub(crate) const SITE_PAGE: &str = "p-site";
pub(crate) const APP_PAGE: &str = "p-app";
pub(crate) const KIT_PAGE: &str = "p-kit";

pub(crate) const DOC: &str = r#"{
  "version": "1.0",
  "children": [],
  "pages": [
    {"id": "p-site", "name": "Site", "children": [
      {"type": "frame", "id": "home", "name": "Home", "screen": "/", "x": 0, "y": 0,
       "width": 400, "height": 800, "children": [
        {"type": "ref", "id": "cta", "ref": "btn", "x": 20, "y": 20},
        {"type": "frame", "id": "hero-link", "name": "Get started", "width": 120, "height": 40,
         "events": {"onTap": [{"replace": "\"/pricing\""}], "onHoverEnter": [{"pop": null}]}},
        {"type": "frame", "id": "old-link", "name": "Old", "width": 80, "height": 40,
         "events": {"onTap": [{"push": "\"/old\""}]}},
        {"type": "frame", "id": "about-link", "name": "About us", "width": 80, "height": 40,
         "events": {"onTap": [{"replace": "\"/about\""}]}},
        {"type": "frame", "id": "sec", "name": "Features", "width": 400, "height": 300},
        {"type": "text", "id": "jump", "content": "See features",
         "events": {"onTap": [{"scroll_to": {"target": "sec", "alignment": "start"}}]}}
      ]},
      {"type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
       "x": 500, "y": 0, "width": 400, "height": 800},
      {"type": "frame", "id": "about", "name": "About", "x": 1000, "y": 0,
       "width": 400, "height": 800}
    ]},
    {"id": "p-app", "name": "App", "children": [
      {"type": "frame", "id": "dash", "name": "Dashboard", "screen": "/", "x": 0, "y": 0,
       "width": 400, "height": 800, "children": [
        {"type": "frame", "id": "to-site", "name": "Pricing link", "width": 80, "height": 40,
         "events": {"onTap": [{"replace": "\"/pricing\""}]}}
      ]}
    ]},
    {"id": "p-kit", "name": "Components", "children": [
      {"type": "frame", "id": "btn", "name": "Button", "reusable": true, "x": 0, "y": 0,
       "width": 160, "height": 48,
       "events": {"onTap": [{"replace": "\"/pricing\""}]},
       "children": [
        {"type": "text", "id": "btn-label", "name": "Label", "content": "Buy"},
        {"type": "frame", "id": "btn-icon", "name": "Icon", "width": 16, "height": 16,
         "events": {"onTap": [{"scroll_to": {"target": "btn-label", "alignment": "start"}}]}}
      ]}
    ]}
  ]
}"#;

pub(crate) fn document() -> PenDocument {
    serde_json::from_str(DOC).expect("fixture parses")
}

/// The fixture opened on the site page.
pub(crate) fn state() -> EditorState {
    EditorState::from_document(document())
}
