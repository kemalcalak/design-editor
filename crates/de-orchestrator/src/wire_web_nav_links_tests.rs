//! Tests for the website header / footer link wiring (#153).

use super::*;
use de_editor_core::EditorState;
use jian_ops_schema::PenDocument;

fn state_from_json(json: &str) -> EditorState {
    let doc: PenDocument = serde_json::from_str(json).expect("valid PenDocument");
    EditorState::from_document(doc)
}

fn run(state: &mut EditorState) {
    let mut sink = crate::loop_finalize::StateDocSink { state };
    super::super::wire_screen_navigation(&mut sink);
    wire_web_nav_links(&mut sink);
}

fn on_tap(state: &EditorState, id: &str) -> Option<serde_json::Value> {
    let node = de_editor_core::walkers::find_node(
        state.active_children(),
        &de_editor_core::NodeId::new(id),
    )?;
    let json = serde_json::to_value(node).ok()?;
    json.get("events")?.get("onTap").cloned()
}

/// Two desktop pages; the header has text links, a button link and a
/// section link; the footer repeats one page link.
const SITE: &str = r#"{"version":"1.0","children":[
    {"type":"frame","id":"home","name":"Home","width":1280,"height":2000,"layout":"vertical",
     "children":[
        {"type":"frame","id":"site-header","name":"Header","layout":"horizontal","children":[
            {"type":"text","id":"logo","content":"Acme"},
            {"type":"frame","id":"links","layout":"horizontal","children":[
                {"type":"text","id":"link-home","content":"Home"},
                {"type":"text","id":"link-pricing","content":"Pricing"},
                {"type":"text","id":"link-features","content":"Features"}
            ]},
            {"type":"frame","id":"cta","name":"Button","children":[
                {"type":"text","id":"cta-label","content":"Pricing"}
            ]}
        ]},
        {"type":"frame","id":"features","name":"Features","height":400,"children":[]},
        {"type":"frame","id":"footer","name":"Footer","children":[
            {"type":"text","id":"footer-pricing","content":"Pricing"},
            {"type":"text","id":"footer-legal","content":"Terms"}
        ]}
     ]},
    {"type":"frame","id":"pricing","name":"Pricing","width":1280,"height":900,"children":[]}
]}"#;

#[test]
fn header_and_footer_links_navigate_to_their_pages() {
    let mut state = state_from_json(SITE);
    run(&mut state);
    let expected = serde_json::json!([{ "replace": "\"/pricing\"" }]);
    assert_eq!(on_tap(&state, "link-pricing"), Some(expected.clone()));
    assert_eq!(on_tap(&state, "footer-pricing"), Some(expected.clone()));
    assert_eq!(
        on_tap(&state, "cta"),
        Some(expected),
        "a button's frame takes the click, not its label"
    );
    assert_eq!(on_tap(&state, "cta-label"), None);
    assert_eq!(
        on_tap(&state, "link-home"),
        Some(serde_json::json!([{ "replace": "\"/\"" }]))
    );
}

#[test]
fn a_label_naming_a_section_scrolls_to_it() {
    let mut state = state_from_json(SITE);
    run(&mut state);
    assert_eq!(
        on_tap(&state, "link-features"),
        Some(serde_json::json!([
            { "scroll_to": { "target": "features", "alignment": "start" } }
        ]))
    );
}

#[test]
fn unmatched_labels_and_text_outside_surfaces_stay_unbound() {
    let mut state = state_from_json(SITE);
    run(&mut state);
    assert_eq!(on_tap(&state, "footer-legal"), None);
    assert_eq!(on_tap(&state, "logo"), None);
}

#[test]
fn an_authored_event_in_the_item_is_kept() {
    let json = SITE.replace(
        r#"{"type":"text","id":"link-pricing","content":"Pricing"}"#,
        r#"{"type":"text","id":"link-pricing","content":"Pricing",
            "events":{"onTap":[{"open_url":{"url":"'https://example.com'"}}]}}"#,
    );
    let mut state = state_from_json(&json);
    run(&mut state);
    assert_eq!(
        on_tap(&state, "link-pricing"),
        Some(serde_json::json!([{ "open_url": { "url": "'https://example.com'" } }]))
    );
}

#[test]
fn running_twice_is_a_no_op_the_second_time() {
    let mut state = state_from_json(SITE);
    run(&mut state);
    let once = serde_json::to_string(&state.doc).unwrap();
    run(&mut state);
    assert_eq!(serde_json::to_string(&state.doc).unwrap(), once);
}

#[test]
fn single_page_site_still_gets_section_links() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"landing","name":"Landing","width":1280,"height":3000,"children":[
            {"type":"frame","id":"nav","name":"Navbar","children":[
                {"type":"text","id":"to-faq","content":"FAQ"}
            ]},
            {"type":"frame","id":"faq","name":"FAQ","children":[]}
        ]}
    ]}"#;
    let mut state = state_from_json(json);
    run(&mut state);
    assert_eq!(
        on_tap(&state, "to-faq"),
        Some(serde_json::json!([
            { "scroll_to": { "target": "faq", "alignment": "start" } }
        ]))
    );
}

/// Realistic sections: "Pricing" holds "Pricing Card"s, and a section has
/// its own "Section Header" whose title must not become a link.
#[test]
fn sections_with_named_children_and_inner_headers_wire_correctly() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"landing","name":"Landing","width":1280,"height":3000,"children":[
            {"type":"frame","id":"top","layout":"vertical","children":[
                {"type":"frame","id":"bar","name":"Top Nav","children":[
                    {"type":"text","id":"to-pricing","content":"Pricing"}
                ]}
            ]},
            {"type":"frame","id":"pricing","name":"Pricing","children":[
                {"type":"frame","id":"pricing-head","name":"Section Header","children":[
                    {"type":"text","id":"pricing-title","content":"Pricing"}
                ]},
                {"type":"frame","id":"card-a","name":"Pricing Card","children":[]},
                {"type":"frame","id":"card-b","name":"Pricing Card","children":[]}
            ]}
        ]}
    ]}"#;
    let mut state = state_from_json(json);
    run(&mut state);
    assert_eq!(
        on_tap(&state, "to-pricing"),
        Some(serde_json::json!([
            { "scroll_to": { "target": "pricing", "alignment": "start" } }
        ])),
        "the shallowest exact match wins; a surface inside an unnamed wrapper counts"
    );
    assert_eq!(
        on_tap(&state, "pricing-title"),
        None,
        "a header deep inside a section is not a nav surface"
    );
}
