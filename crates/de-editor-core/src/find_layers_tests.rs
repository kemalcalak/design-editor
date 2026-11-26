use super::*;
use crate::{EditorState, NodeId, PenNodeExt};

pub(crate) fn multi_page_state() -> EditorState {
    let doc = serde_json::json!({
        "version": "1.0.0",
        "variables": {"primary": {"type": "color", "value": "#2563EB"}},
        "pages": [
            {"id": "p1", "name": "Home", "children": [
                {"type": "frame", "id": "hero", "name": "Hero", "width": 400, "height": 300,
                 "fill": [{"type": "solid", "color": "#2563EB"}],
                 "children": [
                    {"type": "text", "id": "title", "name": "Title", "content": "Sign in to continue"},
                    {"type": "rectangle", "id": "cta", "name": "Sign in button", "width": 120, "height": 40,
                     "fill": [{"type": "solid", "color": "$primary"}]}
                 ]},
                {"type": "frame", "id": "btn-master", "name": "Button", "reusable": true,
                 "width": 120, "height": 40, "children": []}
            ]},
            {"id": "p2", "name": "Settings", "children": [
                {"type": "text", "id": "signin2", "name": "Label", "content": "Sign in again"},
                {"type": "ref", "id": "btn-use", "ref": "btn-master", "x": 0, "y": 0},
                {"type": "text", "id": "locked", "name": "Locked", "content": "sign in", "locked": true}
            ]}
        ]
    });
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn ids(results: &FindResults) -> Vec<&str> {
    results.matches.iter().map(|m| m.node_id.as_str()).collect()
}

#[test]
fn free_text_searches_names_then_text_across_pages() {
    let state = multi_page_state();
    let results = find_layers(&state, &FindQuery::parse("sign in"));
    assert_eq!(results.pages, 2, "matches on both pages");
    // The name hit ranks over the text hits; text hits keep page order.
    assert_eq!(ids(&results), ["cta", "title", "signin2", "locked"]);
    assert_eq!(results.matches[0].field, MatchField::Name);
    assert_eq!(results.matches[1].field, MatchField::Text);
    assert_eq!(
        results.matches[1].snippet.as_deref(),
        Some("Sign in to continue")
    );
    assert_eq!(results.matches[2].page_index, 1);
}

#[test]
fn ranking_prefers_exact_then_prefix_then_contains() {
    let state = multi_page_state();
    let results = find_layers(&state, &FindQuery::parse("button"));
    // "Button" (exact) over "Sign in button" (word start).
    assert_eq!(ids(&results)[..2], ["btn-master", "cta"]);
    assert!(results.matches[0].score > results.matches[1].score);
}

#[test]
fn filters_narrow_by_type_id_fill_variable_and_component() {
    let state = multi_page_state();
    let find = |q: &str| {
        find_layers(&state, &FindQuery::parse(q))
            .matches
            .into_iter()
            .map(|m| m.node_id.as_str().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(find("type:text"), ["title", "signin2", "locked"]);
    assert_eq!(find("sign type:rectangle"), ["cta"]);
    assert_eq!(find("id:signin"), ["signin2"]);
    assert_eq!(find("fill:#2563eb"), ["hero"]);
    assert_eq!(find("fill:2563EBFF"), ["hero"]);
    assert!(find("fill:nonsense").is_empty());
    assert_eq!(find("var:$primary"), ["cta"]);
    assert_eq!(find("component:button"), ["btn-master", "btn-use"]);
    assert_eq!(find("text:sign in again"), ["signin2"]);
    assert!(find("").is_empty(), "an empty query finds nothing");
}

#[test]
fn query_parsing_keeps_free_text_and_multi_word_filters() {
    let q = FindQuery::parse("Hero  type:Frame text:Sign In");
    assert_eq!(q.terms, "hero");
    assert_eq!(q.kind.as_deref(), Some("frame"));
    assert_eq!(q.text.as_deref(), Some("sign in"));
    assert_eq!(q.replace_needle(), Some("sign in"));
    let empty = FindQuery::parse("type:");
    assert!(empty.is_empty());
}

#[test]
fn select_all_matches_switches_page_and_selects_every_match_there() {
    let mut state = multi_page_state();
    let results = find_layers(&state, &FindQuery::parse("type:text"));
    let on_second: Vec<NodeId> = results
        .matches
        .iter()
        .filter(|m| m.page_index == 1)
        .map(|m| m.node_id.clone())
        .collect();
    let selected = state.select_layer_matches(1, &on_second);
    assert_eq!(state.ui.active_page_index, 1);
    assert_eq!(selected.len(), 2);
    assert_eq!(state.selection.set, on_second);
    assert_eq!(state.selection.anchor.as_str(), "locked");
}

#[test]
fn select_one_match_on_another_page() {
    let mut state = multi_page_state();
    assert!(state.select_layer_match(1, &NodeId::new("signin2")));
    assert_eq!(state.ui.active_page_index, 1);
    assert_eq!(state.selection.anchor.as_str(), "signin2");
    assert!(!state.select_layer_match(0, &NodeId::new("missing")));
}

#[test]
fn replace_rewrites_matches_on_every_page_as_one_undo_step() {
    let mut state = multi_page_state();
    let query = FindQuery::parse("sign in");
    let matches: Vec<(usize, NodeId)> = find_layers(&state, &query)
        .matches
        .iter()
        .map(|m| (m.page_index, m.node_id.clone()))
        .collect();
    let count = state.replace_text_in_layer_matches(&query, &matches, "Log in");
    // `cta` matched by name (no text), `locked` is locked.
    assert_eq!(count, 2);
    let text_of = |state: &EditorState, page: usize, id: &str| {
        let pages = state.doc.pages.as_ref().unwrap();
        let node = crate::walkers::find_node(&pages[page].children, &NodeId::new(id)).unwrap();
        node_text_content(node).unwrap()
    };
    assert_eq!(text_of(&state, 0, "title"), "Log in to continue");
    assert_eq!(text_of(&state, 1, "signin2"), "Log in again");
    assert_eq!(text_of(&state, 1, "locked"), "sign in");
    assert!(state.undo());
    assert_eq!(text_of(&state, 0, "title"), "Sign in to continue");
    assert_eq!(text_of(&state, 1, "signin2"), "Sign in again");
    let _ = state.active_children()[0].id_str();
}

#[test]
fn case_insensitive_replace_handles_unicode() {
    assert_eq!(
        replace_case_insensitive("Straße STRASSE", "straße", "x").as_deref(),
        Some("x STRASSE")
    );
    assert_eq!(
        replace_case_insensitive("İstanbul", "i̇stanbul", "Ankara").as_deref(),
        Some("Ankara")
    );
    assert_eq!(replace_case_insensitive("abc", "x", "y"), None);
    assert_eq!(
        replace_case_insensitive("aaa", "a", "b").as_deref(),
        Some("bbb")
    );
}

fn state_of(children: serde_json::Value) -> EditorState {
    let doc = serde_json::json!({"version": "1.0.0", "children": children});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

#[test]
fn text_matches_use_the_raw_text_a_replace_rewrites() {
    let mut state = state_of(serde_json::json!([
        {"type": "text", "id": "split", "name": "A", "content": "Sign\nin"},
        {"type": "text", "id": "one", "name": "B", "content": "Sign  in now"}
    ]));
    // Neither layer holds "sign in" as written, so neither is listed …
    assert!(find_layers(&state, &FindQuery::parse("sign in"))
        .matches
        .is_empty());
    // … and every listed text match is one a replace changes.
    let query = FindQuery::parse("now");
    let all = find_all_layer_matches(&state, &query, None);
    assert_eq!(all.len(), 1);
    assert_eq!(
        state.replace_text_in_layer_matches(&query, &all, "later"),
        all.len()
    );
    // The row shows the text whitespace-collapsed.
    let shown = find_layers(&state, &FindQuery::parse("sign"));
    assert_eq!(shown.matches[0].snippet.as_deref(), Some("Sign in"));
}

#[test]
fn every_match_is_found_past_the_row_cap() {
    let children: Vec<serde_json::Value> = (0..MAX_FIND_RESULTS + 20)
        .map(|i| serde_json::json!({"type": "text", "id": format!("t{i}"), "content": "Buy"}))
        .collect();
    let mut state = state_of(serde_json::Value::Array(children));
    let query = FindQuery::parse("buy");
    let shown = find_layers(&state, &query);
    assert_eq!(shown.matches.len(), MAX_FIND_RESULTS);
    assert_eq!(shown.total, MAX_FIND_RESULTS + 20);
    let all = find_all_layer_matches(&state, &query, None);
    assert_eq!(all.len(), MAX_FIND_RESULTS + 20);
    assert_eq!(
        state.replace_text_in_layer_matches(&query, &all, "Get"),
        MAX_FIND_RESULTS + 20
    );
    assert!(find_all_layer_matches(&state, &query, None).is_empty());
}

#[test]
fn a_match_not_on_its_page_never_switches_pages() {
    let mut state = multi_page_state();
    assert!(!state.select_layer_match(1, &NodeId::new("title")));
    assert_eq!(state.ui.active_page_index, 0, "stayed on the first page");
    assert!(state
        .select_layer_matches(1, &[NodeId::new("title")])
        .is_empty());
    assert_eq!(state.ui.active_page_index, 0);
}
