//! A box marked `escapesClip` leaves the clipping element it sits in.

use jian_ops_schema::node::PenNode;

use crate::{import_snapshot, HtmlImportOptions};

/// A dropdown: an unclipped wrapper holding a rounded `overflow: hidden`
/// menu, whose open submenu's fixed popper sits under a plain item inside
/// the menu and is drawn beside it.
const DROPDOWN: &str = r#"{
  "version": 1,
  "root": { "kind": "element", "tag": "body",
    "rect": { "x": 0, "y": 0, "w": 800, "h": 600 },
    "children": [
      { "kind": "element", "tag": "div",
        "rect": { "x": 20, "y": 40, "w": 200, "h": 80 },
        "styles": { "position": "absolute" },
        "attrs": { "class": "dropdown" },
        "children": [
          { "kind": "element", "tag": "div",
            "rect": { "x": 20, "y": 40, "w": 200, "h": 80 },
            "styles": { "overflow": "hidden", "border-top-left-radius": "6px",
              "border-top-right-radius": "6px", "border-bottom-right-radius": "6px",
              "border-bottom-left-radius": "6px" },
            "attrs": { "class": "menu" },
            "children": [
              { "kind": "text", "rect": { "x": 28, "y": 48, "w": 40, "h": 16 },
                "text": "Share", "lines": 1, "styles": {} },
              { "kind": "element", "tag": "div",
                "rect": { "x": 28, "y": 72, "w": 184, "h": 32 },
                "attrs": { "class": "item" },
                "children": [
                  { "kind": "element", "tag": "div",
                    "rect": { "x": 224, "y": 60, "w": 160, "h": 60 },
                    "styles": { "position": "fixed" },
                    "escapesClip": true,
                    "attrs": { "class": "submenu" },
                    "children": [
                      { "kind": "text", "rect": { "x": 232, "y": 68, "w": 60, "h": 16 },
                        "text": "Email link", "lines": 1, "styles": {} }
                    ] }
                ] }
            ] }
        ] }
    ] }
}"#;

fn named<'a>(nodes: &'a [PenNode], name: &str) -> Option<&'a PenNode> {
    nodes.iter().find_map(|node| match node {
        PenNode::Frame(frame) if frame.base.name.as_deref() == Some(name) => Some(node),
        PenNode::Frame(frame) => named(frame.children.as_deref().unwrap_or_default(), name),
        _ => None,
    })
}

fn children(node: &PenNode) -> &[PenNode] {
    match node {
        PenNode::Frame(frame) => frame.children.as_deref().unwrap_or_default(),
        _ => &[],
    }
}

#[test]
fn an_escaping_popper_leaves_the_clipping_menu() {
    for auto in [false, true] {
        let options = HtmlImportOptions {
            snapshot_auto_layout: auto,
            ..HtmlImportOptions::default()
        };
        let result = import_snapshot(DROPDOWN, &options);
        let dropdown = named(&result.nodes, "Dropdown").expect("dropdown");
        let menu = named(&result.nodes, "Menu").expect("menu");
        assert!(named(children(menu), "Submenu").is_none(), "auto {auto}");
        // In front of the menu, where the page drew it.
        let PenNode::Frame(submenu) = &children(dropdown)[0] else {
            panic!("auto {auto}: submenu first")
        };
        assert_eq!(submenu.base.name.as_deref(), Some("Submenu"));
        assert_eq!((submenu.base.x, submenu.base.y), (Some(204.0), Some(20.0)));
    }
}

#[test]
fn an_unmarked_popper_stays_where_the_document_put_it() {
    let json = DROPDOWN.replace("\"escapesClip\": true,", "");
    let result = import_snapshot(&json, &HtmlImportOptions::default());
    let menu = named(&result.nodes, "Menu").expect("menu");
    assert!(named(children(menu), "Submenu").is_some());
}
