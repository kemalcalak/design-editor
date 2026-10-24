//! The `onTap` patch the wiring passes write: its navigate encoding and
//! the merge that keeps a node's other handlers (#176). Split out of
//! `wire_screen_navigation_tests.rs` at the 800-line cap.

use super::*;
use jian_ops_schema::PenDocument;

fn state_from_json(json: &str) -> EditorState {
    let doc: PenDocument = serde_json::from_str(json).expect("valid PenDocument");
    EditorState::from_document(doc)
}

fn events_of(state: &EditorState, id: &str) -> serde_json::Value {
    let node =
        de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id.to_string()))
            .expect("node");
    serde_json::to_value(node).expect("serializes")["events"].clone()
}

/// Contract point 1: the navigate body must be a Tier-1 string-LITERAL
/// expression source — a bare `/path` lexes as division and fails to
/// compile (`Expression::compile`). The correct wire form is exactly the
/// shape asserted by jian-core's own fixtures
/// (`jian-core/tests/action_navigation.rs::push_literal_path`,
/// `jian-ops-schema/src/events.rs::push_action_with_string_body`): the
/// JSON value under `"replace"`/`"push"` is the STRING `"\"/path\""` (i.e.
/// its decoded content is `"/path"`, quote characters included).
#[test]
fn navigate_patch_produces_expression_literal_body() {
    let patch =
        on_tap_patch(None, navigate_action(NavigateVerb::Replace, "/profile")).expect("serializes");
    assert_eq!(
        patch,
        r#"{"events":{"onTap":[{"replace":"\"/profile\""}]}}"#
    );
    // Round-trip through a real JSON parser to double-check the DECODED
    // action body equals the 10-char string `"/profile"` (quotes included) —
    // exactly what `jian_core::action::actions::navigation::expr_from_value`
    // feeds to `Expression::compile`.
    let v: serde_json::Value = serde_json::from_str(&patch).unwrap();
    let body = v["events"]["onTap"][0]["replace"].as_str().unwrap();
    assert_eq!(body, "\"/profile\"");
}

/// Regression (#176): `PatchNodeData` merges top-level keys only, so the
/// old bare `{"events":{"onTap":…}}` patch replaced the whole `events`
/// block. Wiring a tap now keeps the node's other handlers.
#[test]
fn wiring_a_tap_keeps_sibling_handlers() {
    let json = r#"{"version":"1.0","children":[
        {"type":"frame","id":"root","width":390,"height":844,"children":[
            {"type":"frame","id":"card","width":100,"height":40,
             "events":{"onSwipe":[{"pop":null}],"onChange":[{"pop":null}]}}
        ]}
    ]}"#;
    let mut state = state_from_json(json);
    let mut sink = crate::loop_finalize::StateDocSink { state: &mut state };
    apply_on_tap(
        &mut sink,
        "card",
        navigate_action(NavigateVerb::Push, "/detail"),
    );
    let events = events_of(&state, "card");
    assert_eq!(events["onTap"][0]["push"], "\"/detail\"");
    assert_eq!(events["onSwipe"][0], serde_json::json!({"pop": null}));
    assert_eq!(events["onChange"][0], serde_json::json!({"pop": null}));

    // The pure builder keeps them too.
    let existing: EventHandlers =
        serde_json::from_value(serde_json::json!({"onSwipe":[{"pop":null}]})).unwrap();
    let patch = on_tap_patch(
        Some(&existing),
        de_editor_core::interaction_edit::pop_action(),
    )
    .expect("serializes");
    let v: serde_json::Value = serde_json::from_str(&patch).unwrap();
    assert!(v["events"]["onSwipe"].is_array(), "{patch}");
    assert!(v["events"]["onTap"].is_array(), "{patch}");
}
