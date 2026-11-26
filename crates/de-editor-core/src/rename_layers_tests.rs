use std::collections::BTreeMap;

use serde_json::json;

use super::ai::{parse_rename_layers_reply, rename_layer_briefs, rename_layers_prompt};
use super::*;
use crate::pen_node_ext::PenNodeExt;

fn state() -> EditorState {
    let doc = json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 900,
         "layout": "vertical", "children": [
            {"type": "frame", "id": "hero", "name": "Frame 2", "width": 1440, "height": 600,
             "layout": "vertical", "children": [
                {"type": "text", "id": "title", "name": "Text", "content": "Ship faster", "fontSize": 56},
                {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
                 "fill": [{"type": "solid", "color": "#2563EB"}],
                 "children": [{"type": "text", "id": "cta-label", "content": "Start", "fontSize": 16}]}
             ]},
            {"type": "frame", "id": "kept", "name": "Testimonials", "width": 1440, "height": 300}
         ]},
        {"type": "rectangle", "id": "other", "name": "Rectangle 9", "width": 10, "height": 10}
    ]});
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn name_of(state: &EditorState, id: &str) -> Option<String> {
    crate::walkers::find_node(state.active_children(), &NodeId::new(id))
        .and_then(|node| node.base().name.clone())
}

fn open_on_screen(state: &mut EditorState) -> &RenameLayersDialog {
    state.set_single_selection(NodeId::new("screen"));
    assert!(state.open_rename_layers());
    state.editor_ui.rename_layers.as_ref().expect("dialog open")
}

#[test]
fn opening_previews_the_selection_without_renaming() {
    let mut state = state();
    let dialog = open_on_screen(&mut state);
    let ids: Vec<&str> = dialog.rows.iter().map(|row| row.node_id.as_str()).collect();
    assert!(ids.contains(&"cta") && ids.contains(&"title"));
    assert!(!ids.contains(&"other"), "outside the selection");
    assert!(!ids.contains(&"kept"), "a meaningful name is kept");
    assert!(dialog.ai_requested);
    assert_eq!(dialog.source, RenameLayersSource::Rules);
    // A preview is not an edit.
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert!(!state.is_dirty());
}

#[test]
fn applying_renames_accepted_rows_in_one_undo_step() {
    let mut state = state();
    open_on_screen(&mut state);
    let title_row = state
        .editor_ui
        .rename_layers
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .position(|row| row.node_id.as_str() == "title")
        .expect("title row");
    assert!(state.toggle_rename_layers_row(title_row));
    assert!(state.apply_rename_layers());
    assert!(state.editor_ui.rename_layers.is_none());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Primary button"));
    assert_eq!(name_of(&state, "hero").as_deref(), Some("Hero"));
    assert_eq!(
        name_of(&state, "title").as_deref(),
        Some("Text"),
        "unticked row kept"
    );
    // Ids never change.
    assert!(crate::walkers::find_node(state.active_children(), &NodeId::new("cta")).is_some());
    assert!(state.undo());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
    assert_eq!(name_of(&state, "hero").as_deref(), Some("Frame 2"));
}

#[test]
fn an_edited_name_wins_over_the_agent() {
    let mut state = state();
    open_on_screen(&mut state);
    let rows = &state.editor_ui.rename_layers.as_ref().unwrap().rows;
    let cta = rows
        .iter()
        .position(|row| row.node_id.as_str() == "cta")
        .unwrap();
    assert!(state.start_rename_layers_edit(cta, 0));
    state
        .editor_ui
        .rename_layers
        .as_mut()
        .unwrap()
        .input
        .set_text("");
    for c in "Start button".chars() {
        assert_eq!(rename_layers_text(&mut state, c, 0), Some(true));
    }
    assert_eq!(rename_layers_enter(&mut state), Some(true));
    let mut names = BTreeMap::new();
    names.insert("cta".to_string(), "CTA".to_string());
    names.insert("hero".to_string(), "Landing hero".to_string());
    names.insert("title".to_string(), "Frame 4".to_string());
    state.merge_rename_layers_ai(&names);
    let dialog = state.editor_ui.rename_layers.as_ref().unwrap();
    assert_eq!(dialog.source, RenameLayersSource::Ai);
    let proposed = |id: &str| {
        dialog
            .rows
            .iter()
            .find(|row| row.node_id.as_str() == id)
            .unwrap()
            .proposed
            .clone()
    };
    assert_eq!(proposed("cta"), "Start button");
    assert_eq!(proposed("hero"), "Landing hero");
    assert_ne!(
        proposed("title"),
        "Frame 4",
        "a default name from the agent is ignored"
    );
}

#[test]
fn escape_cancels_the_edit_then_closes_and_keys_never_leak() {
    let mut state = state();
    open_on_screen(&mut state);
    assert!(state.rename_layers_owns_keyboard());
    // Not editing: a letter is swallowed, not a tool switch.
    assert_eq!(rename_layers_text(&mut state, 'r', 0), Some(false));
    assert!(state.start_rename_layers_edit(0, 0));
    assert_eq!(rename_layers_escape(&mut state), Some(true));
    assert!(state
        .editor_ui
        .rename_layers
        .as_ref()
        .unwrap()
        .editing
        .is_none());
    assert_eq!(rename_layers_escape(&mut state), Some(true));
    assert!(state.editor_ui.rename_layers.is_none());
    assert_eq!(rename_layers_escape(&mut state), None);
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Rectangle 3"));
}

#[test]
fn the_agent_brief_and_reply_round_trip() {
    let state = state();
    let proposals = state.rename_layer_proposals(&[NodeId::new("hero")]);
    let briefs = rename_layer_briefs(state.active_children(), &proposals);
    let cta = briefs.iter().find(|b| b["id"] == "cta").expect("cta brief");
    assert_eq!(cta["kind"], "frame");
    assert_eq!(cta["suggested"], "Primary button");
    assert_eq!(cta["path"], "Frame 1 / Frame 2");
    assert_eq!(cta["children"][0], "text \"Start\"");
    let prompt = rename_layers_prompt(&briefs, Locale::Tr);
    assert!(prompt.contains("Türkçe (tr)"));
    assert!(prompt.contains("must not share a name"));
    assert!(prompt.contains("\"cta\""));

    let reply = "Sure!\n```json\n{\"cta\": \"Start button\", \"hero\": \" \"}\n```";
    let names = parse_rename_layers_reply(reply);
    assert_eq!(names.get("cta").map(String::as_str), Some("Start button"));
    assert!(!names.contains_key("hero"));
    let array = parse_rename_layers_reply(r#"[{"id":"cta","name":"Go"}]"#);
    assert_eq!(array.get("cta").map(String::as_str), Some("Go"));
    assert!(parse_rename_layers_reply("no json here").is_empty());
}

#[test]
fn a_failed_agent_keeps_the_rules() {
    let mut state = state();
    open_on_screen(&mut state);
    state.merge_rename_layers_ai(&BTreeMap::new());
    assert_eq!(
        state.editor_ui.rename_layers.as_ref().unwrap().source,
        RenameLayersSource::AiFailed
    );
    state.fail_rename_layers_ai();
    assert!(state.apply_rename_layers());
    assert_eq!(name_of(&state, "cta").as_deref(), Some("Primary button"));
}

#[test]
fn reopening_an_open_preview_keeps_its_edits() {
    let mut state = state();
    open_on_screen(&mut state);
    assert!(state.toggle_rename_layers_row(0));
    assert!(!state.open_rename_layers(), "already open");
    assert!(!state.editor_ui.rename_layers.as_ref().unwrap().rows[0].accepted);
}

#[test]
fn instances_are_named_after_the_content_their_master_shows() {
    // A plan-card master on the page and an instance that overrides the
    // plan name: the instance reads its own override.
    let doc = json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "plan", "name": "Plan", "reusable": true, "width": 240,
         "height": 200, "cornerRadius": 12, "layout": "vertical",
         "fill": [{"type": "solid", "color": "#FFFFFF"}], "children": [
            {"type": "text", "id": "plan-name", "content": "Starter", "fontSize": 20},
            {"type": "text", "id": "plan-price", "content": "$9", "fontSize": 28}
         ]},
        {"type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 900,
         "layout": "vertical", "children": [
            {"type": "ref", "id": "pro", "name": "Frame 4", "ref": "plan",
             "descendants": {"plan-name": {"content": "Pro"}}}
         ]}
    ]});
    let state = EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    let proposals = state.rename_layer_proposals(&[NodeId::new("pro")]);
    assert_eq!(proposals.len(), 1, "{proposals:?}");
    assert_eq!(proposals[0].node_id, "pro");
    assert_eq!(proposals[0].proposed, "Pro card");
}
