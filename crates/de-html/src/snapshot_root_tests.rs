//! Tests for the whole-page root box and the expanded-scroll flag.

use super::*;
use crate::HtmlImportOptions;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

/// A full-screen app shell: everything sits in one `position: fixed;
/// inset: 0` overlay, so the body and its `#app` wrapper report 0 px tall.
const FIXED_SHELL: &str = r#"{
  "version": 1,
  "viewport": { "width": 1280, "height": 800 },
  "root": {
    "kind": "element", "tag": "body",
    "rect": { "x": 0, "y": 0, "w": 1280, "h": 0 },
    "children": [
      { "kind": "element", "tag": "div",
        "rect": { "x": 0, "y": 0, "w": 1280, "h": 0 },
        "children": [
          { "kind": "element", "tag": "div",
            "rect": { "x": 0, "y": 0, "w": 1280, "h": 800 },
            "styles": { "position": "fixed", "background-color": "rgb(10, 10, 10)" },
            "children": [] }
        ] }
    ]
  }
}"#;

fn frame(node: &PenNode) -> &jian_ops_schema::node::FrameNode {
    let PenNode::Frame(frame) = node else {
        panic!("expected a frame, got {node:?}")
    };
    frame
}

fn size(frame: &jian_ops_schema::node::FrameNode) -> (Option<f64>, Option<f64>) {
    let number = |sizing: &Option<SizingBehavior>| match sizing {
        Some(SizingBehavior::Number(value)) => Some(*value),
        _ => None,
    };
    (
        number(&frame.container.width),
        number(&frame.container.height),
    )
}

#[test]
fn a_fixed_overlay_page_gives_the_root_and_its_wrapper_the_viewport() {
    for auto_layout in [false, true] {
        let options = HtmlImportOptions {
            snapshot_auto_layout: auto_layout,
            ..HtmlImportOptions::default()
        };
        let result = import_snapshot(FIXED_SHELL, &options);
        let root = frame(&result.nodes[0]);
        if auto_layout {
            // The page root hugs its one in-flow child, now 800 tall.
            assert_eq!(size(root).0, Some(1280.0));
            assert!(matches!(
                root.container.height,
                Some(SizingBehavior::Keyword(SizingKeyword::FitContent))
            ));
        } else {
            assert_eq!(size(root), (Some(1280.0), Some(800.0)));
        }
        let wrapper = frame(&root.children.as_ref().unwrap()[0]);
        // Auto layout lets the wrapper fill the page's width.
        let fills = matches!(
            wrapper.container.width,
            Some(SizingBehavior::Keyword(SizingKeyword::FillContainer))
        );
        if auto_layout {
            assert!(fills, "auto layout: the wrapper fills the page's width");
        } else {
            assert_eq!(size(wrapper).0, Some(1280.0));
        }
        assert_eq!(size(wrapper).1, Some(800.0), "auto: {auto_layout}");
        let overlay = frame(&wrapper.children.as_ref().unwrap()[0]);
        assert_eq!(
            size(overlay),
            (Some(1280.0), Some(800.0)),
            "auto: {auto_layout}"
        );
    }
}

#[test]
fn absolute_content_below_an_empty_body_extends_the_page() {
    let json = r#"{
      "version": 1,
      "viewport": { "width": 1000, "height": 600 },
      "root": {
        "kind": "element", "tag": "body",
        "rect": { "x": 0, "y": 0, "w": 1000, "h": 0 },
        "children": [
          { "kind": "element", "tag": "footer",
            "rect": { "x": 0, "y": 900, "w": 1000, "h": 100 },
            "styles": { "position": "absolute" }, "children": [] },
          { "kind": "element", "tag": "nav",
            "rect": { "x": 0, "y": 0, "w": 200, "h": 300 },
            "styles": { "position": "absolute", "overflow": "hidden auto" },
            "children": [
              { "kind": "element", "tag": "ul",
                "rect": { "x": 0, "y": 0, "w": 200, "h": 5000 }, "children": [] }
            ] },
          { "kind": "element", "tag": "aside",
            "rect": { "x": 0, "y": 2000, "w": 300, "h": 600 },
            "styles": { "position": "fixed" }, "children": [] }
        ]
      }
    }"#;
    let result = import_snapshot(json, &HtmlImportOptions::default());
    let root = frame(&result.nodes[0]);
    // The footer reaches 1000; the fixed drawer parked off screen and the
    // list the nav scrolls do not count.
    assert_eq!(size(root), (Some(1000.0), Some(1000.0)));
}

/// A body with content keeps its own box: the viewport is not the page's
/// width (scrollbar, body margin), and a short page still hugs its content.
#[test]
fn a_page_with_content_keeps_its_body_box() {
    let json = r#"{
      "version": 1,
      "viewport": { "width": 1440, "height": 900 },
      "root": {
        "kind": "element", "tag": "body",
        "rect": { "x": 8, "y": 8, "w": 1409, "h": 200 },
        "children": [
          { "kind": "element", "tag": "main",
            "rect": { "x": 8, "y": 8, "w": 1409, "h": 200 }, "children": [] },
          { "kind": "element", "tag": "div",
            "rect": { "x": 8, "y": 208, "w": 1409, "h": 0 },
            "children": [
              { "kind": "element", "tag": "div",
                "rect": { "x": 0, "y": 0, "w": 1440, "h": 900 },
                "styles": { "position": "fixed" }, "children": [] }
            ] }
        ]
      }
    }"#;
    let result = import_snapshot(json, &HtmlImportOptions::default());
    let root = frame(&result.nodes[0]);
    assert_eq!(size(root), (Some(1409.0), Some(200.0)));
    // The overlay's mount sits beside `main`, not alone, so it keeps 0.
    let mount = root
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(frame)
        .find(|child| child.children.as_ref().is_some_and(|kids| !kids.is_empty()))
        .unwrap();
    assert_eq!(size(mount).1, Some(0.0));
}

#[test]
fn a_subtree_capture_keeps_the_picked_elements_box() {
    let json = FIXED_SHELL.replace(r#""tag": "body""#, r#""tag": "main""#);
    let result = import_snapshot(&json, &HtmlImportOptions::default());
    let root = frame(&result.nodes[0]);
    assert_eq!(size(root), (Some(1280.0), Some(0.0)));
}

#[test]
fn an_expanded_scroll_capture_does_not_warn_about_clipped_scroll_content() {
    let json = r#"{
      "version": 1,
      __FLAG__
      "root": {
        "kind": "element", "tag": "div",
        "rect": { "x": 0, "y": 0, "w": 400, "h": 400 },
        "children": [
          { "kind": "element", "tag": "div",
            "rect": { "x": 0, "y": 0, "w": 400, "h": 100 },
            "styles": { "overflow": "auto" },
            "children": [
              { "kind": "element", "tag": "div",
                "rect": { "x": 0, "y": 0, "w": 400, "h": 300 }, "children": [] }
            ] }
        ]
      }
    }"#;
    let warns = |flag: &str| {
        import_snapshot(
            &json.replace("__FLAG__", flag),
            &HtmlImportOptions::default(),
        )
        .warnings
        .iter()
        .any(|warning| *warning == ImportWarning::OverflowScrollClipped.to_string())
    };
    assert!(warns(""), "a clipped scroll container is reported");
    assert!(!warns(r#""expandedScroll": true,"#));
}
