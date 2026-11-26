use super::*;
use crate::request_snapshot::{narrowed_snapshot, shared_snapshot_pages};

/// Five pages: a component page, a screen using it, a screen whose
/// instance targets a plain (non-reusable) node on the "legacy" page, a
/// screen with an instance whose override embeds another instance, and an
/// unrelated page.
fn state() -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(
        r#"{"version":"1.0","children":[],"pages":[
          {"id":"components","name":"Components","children":[
            {"type":"frame","id":"card","name":"Card","reusable":true,"width":100,"height":40,"children":[
              {"type":"text","id":"card-label","content":"Card"}]}]},
          {"id":"screen","name":"Screen","children":[
            {"type":"frame","id":"s-root","width":400,"height":400,"children":[
              {"type":"ref","id":"s-card","ref":"card"}]}]},
          {"id":"legacy-screen","name":"Legacy screen","children":[
            {"type":"ref","id":"l-inst","ref":"plain-target"}]},
          {"id":"legacy","name":"Legacy","children":[
            {"type":"frame","id":"plain-target","width":10,"height":10}]},
          {"id":"other","name":"Other","children":[
            {"type":"frame","id":"o-root","width":10,"height":10,"children":[
              {"type":"ref","id":"o-card","ref":"card","descendants":{
                "card-label":{"type":"ref","id":"nested","ref":"nested-target"}}}]}]},
          {"id":"nested-home","name":"Nested home","children":[
            {"type":"rectangle","id":"nested-target","width":5,"height":5}]},
          {"id":"unrelated","name":"Unrelated","children":[
            {"type":"rectangle","id":"u","width":5,"height":5}]}
        ]}"#,
    )
    .expect("document");
    EditorState::from_document(doc)
}

fn kept(state: &EditorState, pages: SnapshotPages) -> Option<Vec<bool>> {
    kept_pages(state, &pages, &PageNeeds::of(state))
}

#[test]
fn masters_and_ref_targets_are_always_kept() {
    let mut state = state();
    state.ui.active_page_index = 6;
    // components (master), legacy (plain ref target), nested-home (a ref
    // inside an override) are needed by any page; the active page is added.
    assert_eq!(
        kept(&state, SnapshotPages::Page(None)),
        Some(vec![true, false, false, true, false, true, true])
    );
    assert_eq!(
        kept(&state, SnapshotPages::Page(Some("screen".into()))),
        Some(vec![true, true, false, true, false, true, false])
    );
    assert_eq!(
        kept(&state, SnapshotPages::NoNodes),
        Some(vec![false; 7]),
        "a document-level read keeps no nodes"
    );
}

#[test]
fn everything_is_kept_when_the_scope_cannot_be_narrowed() {
    let state = state();
    assert_eq!(kept(&state, SnapshotPages::All), None);
    assert_eq!(
        kept(&state, SnapshotPages::Page(Some("no-such-page".into()))),
        None,
        "an unknown page reads the whole document, so the tool's own error is unchanged"
    );
    let single: jian_ops_schema::PenDocument = serde_json::from_str(
        r#"{"version":"1.0","children":[{"type":"rectangle","id":"r","width":1,"height":1}]}"#,
    )
    .expect("document");
    let single = EditorState::from_document(single);
    assert_eq!(kept(&single, SnapshotPages::Page(None)), None);
    // Needs taken from another document shape are not trusted.
    let stale = PageNeeds::of(&single);
    assert_eq!(kept_pages(&state, &SnapshotPages::NoNodes, &stale), None);
}

#[test]
fn needs_follow_the_document_revision() {
    let mut state = state();
    let needs = PageNeeds::of(&state);
    assert!(needs.is_current(&state));
    state.commit_history();
    assert!(state.apply(crate::EditorCommand::SetNodeName {
        node_id: crate::NodeId::new("card-label"),
        name: "Renamed".into(),
    }));
    assert!(!needs.is_current(&state));
}

#[test]
fn a_partial_snapshot_carries_the_kept_pages_whole_and_the_rest_empty() {
    let mut state = state();
    state.ui.active_page_index = 1;
    let keep = kept(&state, SnapshotPages::Page(None)).expect("narrowed");
    let full = narrowed_snapshot(&mut state);
    let partial = shared_snapshot_pages(&mut state, None, Some(&keep)).into_state();
    let full_pages = full.doc.pages.as_ref().expect("pages");
    let partial_pages = partial.doc.pages.as_ref().expect("pages");
    assert_eq!(full_pages.len(), partial_pages.len(), "page list is whole");
    for (index, (full_page, partial_page)) in full_pages.iter().zip(partial_pages).enumerate() {
        assert_eq!(full_page.id, partial_page.id);
        assert_eq!(full_page.name, partial_page.name);
        if keep[index] {
            assert_eq!(full_page.children, partial_page.children, "page {index}");
        } else {
            assert!(partial_page.children.is_empty(), "page {index} left out");
        }
    }
    assert_eq!(partial.doc.variables, full.doc.variables);
    assert_eq!(partial.ui.active_page_index, full.ui.active_page_index);
}

#[test]
fn a_merged_anchor_keeps_left_out_pages_and_takes_captured_ones() {
    let mut state = state();
    let full = crate::history_snapshot::SharedDoc::capture(&state.doc, None);
    let keep = kept(&state, SnapshotPages::Page(Some("unrelated".into()))).expect("narrowed");
    let partial = shared_snapshot_pages(&mut state, Some(&full), Some(&keep));
    let merged = full.merged_pages(partial.shared_doc(), &keep);
    for index in 0..7 {
        let expected = if keep[index] {
            partial.shared_doc().page_children(index)
        } else {
            full.page_children(index)
        };
        let got = merged.page_children(index).expect("page");
        let expected = expected.expect("page");
        assert_eq!(got.len(), expected.len());
        for (a, b) in got.iter().zip(expected) {
            assert!(
                std::sync::Arc::ptr_eq(a, b),
                "page {index} shares its nodes"
            );
        }
    }
    assert_eq!(merged.materialize(), state.doc, "merging loses nothing");
}
