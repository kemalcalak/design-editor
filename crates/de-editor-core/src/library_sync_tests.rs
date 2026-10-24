//! Tests for `library_sync.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use jian_ops_schema::node::PenNode;
use jian_ops_schema::variable::VariableDefinition;

use crate::library_link::{LibraryCommand, LibraryContent, LibraryPanelAction, LibraryStatus};
use crate::page_mutators::COMPONENTS_PAGE_NAME;
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, LibraryRequest};

fn node(json: serde_json::Value) -> PenNode {
    serde_json::from_value(json).expect("node")
}

fn master(id: &str, label: &str) -> PenNode {
    node(serde_json::json!({
        "id": id, "type": "frame", "name": id, "reusable": true,
        "width": 100, "height": 40,
        "children": [{ "id": format!("{id}-t"), "type": "text", "content": label }],
    }))
}

fn color(hex: &str) -> VariableDefinition {
    serde_json::from_value(serde_json::json!({ "type": "color", "value": hex })).unwrap()
}

fn content(masters: Vec<PenNode>, vars: &[(&str, &str)]) -> Arc<LibraryContent> {
    let variables: BTreeMap<String, VariableDefinition> = vars
        .iter()
        .map(|(name, hex)| (name.to_string(), color(hex)))
        .collect();
    Arc::new(LibraryContent {
        masters,
        variables,
        themes: BTreeMap::new(),
    })
}

fn doc_with(children: serde_json::Value) -> EditorState {
    let doc = serde_json::json!({ "version": "1.0", "children": children });
    EditorState::from_document(serde_json::from_value(doc).unwrap())
}

fn components_page(state: &EditorState) -> Vec<String> {
    state
        .doc
        .pages
        .as_ref()
        .and_then(|pages| pages.iter().find(|p| p.name == COMPONENTS_PAGE_NAME))
        .map(|p| p.children.iter().map(|n| n.id_str().to_string()).collect())
        .unwrap_or_default()
}

fn label_of(state: &EditorState, id: &str) -> String {
    let page = state
        .doc
        .pages
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| p.name == COMPONENTS_PAGE_NAME)
        .unwrap();
    let master = page.children.iter().find(|n| n.id_str() == id).unwrap();
    let text = &master.children().unwrap()[0];
    match text {
        PenNode::Text(t) => serde_json::to_value(&t.content).unwrap().to_string(),
        _ => panic!("text child"),
    }
}

fn sync(state: &mut EditorState, path: &str, c: Arc<LibraryContent>) {
    assert!(state.apply_library_command(LibraryCommand::Sync {
        path: path.into(),
        alias: None,
        content: c,
    }));
}

#[test]
fn linking_caches_masters_and_resolves_instances() {
    let mut state = doc_with(serde_json::json!([
        { "id": "inst", "type": "ref", "ref": "btn", "x": 0, "y": 0 }
    ]));
    sync(
        &mut state,
        "kit.de",
        content(vec![master("btn", "Go")], &[]),
    );
    assert_eq!(components_page(&state), vec!["btn"]);
    assert!(state
        .components
        .find_by_id(&crate::NodeId::new("btn"))
        .is_some());
    let link = &state.editor_ui.libraries.entries[0];
    assert_eq!(link.link.masters, vec!["btn"]);
    assert_eq!(
        link.status,
        LibraryStatus::Synced {
            components: 1,
            conflicts: 0
        }
    );
    assert_eq!(
        state.ui.active_page_index, 0,
        "the design page stays active"
    );
}

#[test]
fn resync_replaces_owned_masters_and_drops_removed_ones() {
    let mut state = doc_with(serde_json::json!([]));
    sync(
        &mut state,
        "kit.de",
        content(vec![master("a", "One"), master("b", "Two")], &[]),
    );
    sync(&mut state, "kit.de", content(vec![master("a", "Uno")], &[]));
    assert_eq!(components_page(&state), vec!["a"]);
    assert!(label_of(&state, "a").contains("Uno"));
    let revision = state.revision;
    sync(&mut state, "kit.de", content(vec![master("a", "Uno")], &[]));
    assert_eq!(state.revision, revision, "unchanged content is a no-op");
}

#[test]
fn a_local_node_with_the_same_id_wins() {
    let mut state = doc_with(serde_json::json!([
        { "id": "a", "type": "frame", "width": 10, "height": 10 }
    ]));
    sync(
        &mut state,
        "kit.de",
        content(vec![master("a", "x"), master("b", "y")], &[]),
    );
    assert_eq!(components_page(&state), vec!["b"]);
    assert_eq!(
        state.editor_ui.libraries.entries[0].status,
        LibraryStatus::Synced {
            components: 1,
            conflicts: 1
        }
    );
}

#[test]
fn owned_variables_follow_the_library_and_local_ones_stay() {
    let mut state = doc_with(serde_json::json!([]));
    state
        .doc
        .variables
        .get_or_insert_with(Default::default)
        .insert("--local".into(), color("#000000"));
    sync(
        &mut state,
        "kit.de",
        content(vec![], &[("--brand", "#111111"), ("--local", "#FFFFFF")]),
    );
    let vars = state.doc.variables.as_ref().unwrap();
    assert_eq!(vars["--local"], color("#000000"));
    assert_eq!(vars["--brand"], color("#111111"));
    sync(
        &mut state,
        "kit.de",
        content(vec![], &[("--brand", "#222222")]),
    );
    assert_eq!(
        state.doc.variables.as_ref().unwrap()["--brand"],
        color("#222222")
    );
    sync(&mut state, "kit.de", content(vec![], &[]));
    assert!(!state
        .doc
        .variables
        .as_ref()
        .unwrap()
        .contains_key("--brand"));
}

#[test]
fn an_unavailable_library_keeps_its_cached_masters() {
    let mut state = doc_with(serde_json::json!([]));
    sync(&mut state, "kit.de", content(vec![master("a", "x")], &[]));
    assert!(state.apply_library_command(LibraryCommand::Unavailable {
        path: "kit.de".into(),
        status: LibraryStatus::Missing,
    }));
    assert_eq!(components_page(&state), vec!["a"]);
    assert!(state.editor_ui.libraries.entries[0].status.is_problem());
}

#[test]
fn unlink_keeps_used_masters_as_local_components() {
    let mut state = doc_with(serde_json::json!([
        { "id": "inst", "type": "ref", "ref": "card", "x": 0, "y": 0 }
    ]));
    let card = node(serde_json::json!({
        "id": "card", "type": "frame", "reusable": true, "width": 10, "height": 10,
        "children": [{ "id": "card-btn", "type": "ref", "ref": "btn" }],
    }));
    sync(
        &mut state,
        "kit.de",
        content(vec![card, master("btn", "x"), master("unused", "y")], &[]),
    );
    assert!(state.press_library_action(LibraryPanelAction::Unlink("kit.de".into())));
    assert!(state.editor_ui.libraries.entries.is_empty());
    let mut kept = components_page(&state);
    kept.sort();
    assert_eq!(kept, vec!["btn", "card"]);
}

#[test]
fn undo_cannot_roll_a_library_back() {
    let mut state = doc_with(serde_json::json!([
        { "id": "box", "type": "frame", "width": 10, "height": 10 }
    ]));
    sync(&mut state, "kit.de", content(vec![master("a", "Old")], &[]));
    let snap = state.snapshot_for_history();
    state.history_push_past(snap);
    sync(&mut state, "kit.de", content(vec![master("a", "New")], &[]));
    assert!(state.undo());
    assert!(label_of(&state, "a").contains("New"));
}

#[test]
fn link_and_reload_queue_a_host_request() {
    let mut state = EditorState::new();
    state.press_library_action(LibraryPanelAction::Link);
    assert_eq!(
        state.editor_ui.libraries.request,
        Some(LibraryRequest::Link)
    );
    state.press_library_action(LibraryPanelAction::Reload);
    assert_eq!(
        state.editor_ui.libraries.request,
        Some(LibraryRequest::Reload)
    );
}

#[test]
fn relinking_after_unlink_adopts_the_kept_copies() {
    let mut state = doc_with(serde_json::json!([
        { "id": "inst", "type": "ref", "ref": "a", "x": 0, "y": 0 }
    ]));
    sync(&mut state, "kit.de", content(vec![master("a", "One")], &[]));
    state.unlink_library("kit.de");
    sync(&mut state, "kit.de", content(vec![master("a", "Two")], &[]));
    assert_eq!(
        state.editor_ui.libraries.entries[0].status,
        LibraryStatus::Synced {
            components: 1,
            conflicts: 0
        }
    );
    assert!(label_of(&state, "a").contains("Two"));
}

#[test]
fn a_master_the_library_dropped_stays_while_used() {
    let mut state = doc_with(serde_json::json!([
        { "id": "inst", "type": "ref", "ref": "a", "x": 0, "y": 0 }
    ]));
    sync(
        &mut state,
        "kit.de",
        content(vec![master("a", "x"), master("b", "y")], &[]),
    );
    sync(&mut state, "kit.de", content(vec![master("b", "y")], &[]));
    let mut ids = components_page(&state);
    ids.sort();
    assert_eq!(ids, vec!["a", "b"]);
    assert_eq!(state.editor_ui.libraries.entries[0].link.masters, vec!["b"]);
}

#[test]
fn linking_and_unlinking_mark_the_document_dirty() {
    let mut state = doc_with(serde_json::json!([]));
    state.mark_saved_revision();
    sync(&mut state, "kit.de", content(vec![], &[]));
    assert!(state.is_dirty());
    state.mark_saved_revision();
    state.unlink_library("kit.de");
    assert!(state.is_dirty());
}
