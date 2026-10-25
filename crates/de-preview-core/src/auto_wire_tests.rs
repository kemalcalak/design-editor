//! Tests for Track C-1's enter-preview auto-wiring.

use super::*;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

fn parse(json: &str) -> PenDocument {
    serde_json::from_str(json).expect("valid PenDocument")
}

fn frame_screen<'a>(doc: &'a PenDocument, id: &str) -> Option<&'a str> {
    doc.children.iter().find_map(|n| match n {
        PenNode::Frame(f) if f.base.id == id => f.screen.as_deref(),
        _ => None,
    })
}

fn two_unmarked_screens() -> &'static str {
    r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#
}

#[test]
fn unmarked_multi_screen_doc_gets_wired() {
    let doc = parse(two_unmarked_screens());
    let wired = auto_wire_for_preview(&doc, 0, None).expect("no marker -> pass runs");
    assert_eq!(frame_screen(&wired, "home"), Some("/"));
    assert_eq!(frame_screen(&wired, "profile"), Some("/profile"));
}

#[test]
fn caller_document_is_never_mutated() {
    let doc = parse(two_unmarked_screens());
    let before = serde_json::to_string(&doc).unwrap();
    let _ = auto_wire_for_preview(&doc, 0, None);
    let after = serde_json::to_string(&doc).unwrap();
    assert_eq!(after, before, "auto_wire_for_preview must not touch `doc`");
}

#[test]
fn any_authored_marker_skips_the_whole_pass() {
    // "profile" already carries an authored marker — Track C-1's stricter
    // rule (unlike Track A's own per-node gate) skips wiring ENTIRELY, so
    // "home" is left unmarked too, not auto-assigned the entry path.
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile","width":390,"height":844,
         "layout":"vertical","children":[],"screen":"/profile"}
    ]}"#;
    let doc = parse(json);
    assert!(
        auto_wire_for_preview(&doc, 0, None).is_none(),
        "an authored marker anywhere on the page must skip the whole pass"
    );
}

#[test]
fn single_screen_doc_is_untouched() {
    // Only one screen-shaped top-level frame: Track A's own gate (<2
    // candidates) makes the pass a no-op, so auto_wire's `Some` (the pass
    // DID run, unmarked) still yields a byte-identical document.
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":390,"height":844,
         "layout":"vertical","children":[]}
    ]}"#;
    let doc = parse(json);
    let wired = auto_wire_for_preview(&doc, 0, None).expect("no marker -> pass runs");
    assert_eq!(
        serde_json::to_string(&wired).unwrap(),
        serde_json::to_string(&doc).unwrap(),
        "single-screen doc: pass runs but is a no-op"
    );
}

/// Two unmarked desktop pages with a website header (#153).
fn unmarked_site() -> &'static str {
    r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","width":1280,"height":1600,
         "layout":"vertical","children":[
            {"type":"frame","id":"header","name":"Header","layout":"horizontal","children":[
                {"type":"text","id":"to-pricing","content":"Pricing"},
                {"type":"text","id":"to-faq","content":"FAQ"}
            ]},
            {"type":"frame","id":"faq","name":"FAQ","height":300,"children":[]}
         ]},
        {"type":"frame","id":"pricing","name":"Pricing","width":1280,"height":900,
         "layout":"vertical","children":[]}
    ]}"#
}

fn on_tap(doc: &PenDocument, id: &str) -> Option<serde_json::Value> {
    use de_editor_core::PenNodeExt;
    fn find<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
        nodes.iter().find_map(|node| {
            if node.id_str() == id {
                return Some(node);
            }
            find(node.children()?, id)
        })
    }
    let node = find(&doc.children, id)?;
    serde_json::to_value(node)
        .ok()?
        .get("events")?
        .get("onTap")
        .cloned()
}

#[test]
fn website_header_links_are_wired_to_pages_and_sections() {
    let doc = parse(unmarked_site());
    let wired = auto_wire_for_preview(&doc, 0, None).expect("no marker -> pass runs");
    assert_eq!(
        on_tap(&wired, "to-pricing"),
        Some(serde_json::json!([{ "replace": "\"/pricing\"" }]))
    );
    assert_eq!(
        on_tap(&wired, "to-faq"),
        Some(serde_json::json!([
            { "scroll_to": { "target": "faq", "alignment": "start" } }
        ]))
    );
    assert_eq!(
        on_tap(&doc, "to-pricing"),
        None,
        "the caller's doc is untouched"
    );
}

#[test]
fn an_authored_marker_also_skips_website_links() {
    let json = unmarked_site().replace(
        r#""id":"pricing","name":"Pricing","#,
        r#""id":"pricing","name":"Pricing","screen":"/pricing","#,
    );
    assert!(auto_wire_for_preview(&parse(&json), 0, None).is_none());
}

/// #223: with an entry board, instances are expanded and an authored
/// marker no longer switches the pass off.
#[test]
fn an_entry_board_fills_gaps_around_authored_markers() {
    let doc = parse(
        r#"{"version":"1.0","children":[
        {"type":"frame","id":"home","name":"Home","reusable":true,"width":390,"height":844,
         "layout":"vertical","children":[]},
        {"type":"frame","id":"profile","name":"Profile","screen":"/me","width":390,
         "height":844,"layout":"vertical","children":[]},
        {"type":"ref","id":"home-dark","ref":"home","x":500,"y":0}
    ]}"#,
    );
    assert!(auto_wire_for_preview(&doc, 0, None).is_none(), "old rule");
    let wired = auto_wire_for_preview(&doc, 0, Some("home")).expect("pass runs");
    assert_eq!(frame_screen(&wired, "home"), Some("/"));
    assert_eq!(frame_screen(&wired, "profile"), Some("/me"));
    // The instance is expanded into a frame and is not part of the site.
    assert_eq!(frame_screen(&wired, "home-dark"), None);
    assert!(matches!(
        wired
            .children
            .iter()
            .find(|n| matches!(n, PenNode::Frame(f) if f.base.id == "home-dark")),
        Some(PenNode::Frame(_))
    ));
}
