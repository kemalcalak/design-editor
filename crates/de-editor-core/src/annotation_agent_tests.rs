use super::*;
use crate::chat::ChatMessage;

fn state() -> EditorState {
    let doc = serde_json::json!({
        "version": "1.0.0",
        "children": [
            {"type": "frame", "id": "hero", "name": "Hero", "x": 0, "y": 0, "width": 400, "height": 300},
            {"type": "frame", "id": "pricing", "name": "Pricing", "x": 0, "y": 400, "width": 400, "height": 300},
            {"type": "frame", "id": "far", "name": "Far away", "x": 5000, "y": 5000, "width": 100, "height": 100},
            {"type": "note", "id": "n1", "name": "Review", "x": 380, "y": 100, "width": 240, "height": 160,
             "content": "Make the headline calmer"},
            {"type": "prompt", "id": "p1", "x": 2000, "y": 0, "width": 240, "height": 160,
             "content": "Add a footer"},
            {"type": "context", "id": "c1", "x": 9000, "y": 9000, "width": 240, "height": 160,
             "content": "Audience: runners"},
            {"type": "note", "id": "n2", "x": 395, "y": 90, "width": 10, "height": 10}
        ]
    });
    EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    )
}

fn find<'a>(state: &'a EditorState, id: &str) -> &'a PenNode {
    crate::walkers::find_node(state.active_children(), &NodeId::new(id)).unwrap()
}

fn ids(nodes: Vec<&PenNode>) -> Vec<&str> {
    nodes.into_iter().map(|n| n.id_str()).collect()
}

#[test]
fn layers_touching_the_note_come_first_and_annotations_never_count() {
    let state = state();
    let page = state.active_children();
    // The note overlaps Hero's right edge; the other note is ignored.
    assert_eq!(ids(surrounding_layers(page, find(&state, "n1"))), ["hero"]);
    // Nothing touches the prompt and no frame is within reach.
    assert!(surrounding_layers(page, find(&state, "p1")).is_empty());
    // A lone context note far from everything gets nothing.
    assert!(surrounding_layers(page, find(&state, "c1")).is_empty());
}

#[test]
fn nearest_layer_stands_in_when_none_touches() {
    let mut state = state();
    // Move the prompt near Pricing, but not touching it.
    let note =
        crate::walkers::find_node_mut(state.active_children_mut(), &NodeId::new("p1")).unwrap();
    note.base_mut().x = Some(520.0);
    note.base_mut().y = Some(450.0);
    let page = state.active_children();
    assert_eq!(
        ids(surrounding_layers(page, find(&state, "p1"))),
        ["pricing"]
    );
}

#[test]
fn send_opens_the_chat_with_the_note_and_its_layers_staged() {
    let mut state = state();
    assert!(state.is_annotation(&NodeId::new("n1")));
    assert!(!state.is_annotation(&NodeId::new("hero")));
    assert!(state.send_annotation_to_agent(&NodeId::new("n1"), 0));
    // The note's text is in the draft once; the note is not staged again.
    assert_eq!(state.chat.input.text(), "Make the headline calmer\n\n@Hero");
    let staged: Vec<(&str, MentionKind)> = state
        .chat
        .pending_mentions
        .iter()
        .map(|m| (m.key.as_str(), m.kind))
        .collect();
    assert_eq!(staged, [("hero", MentionKind::Node)]);
    assert!(state.chat.focused);
    // Every staged mention's token is in the draft, so the sync keeps them.
    state.sync_chat_mentions();
    assert_eq!(state.chat.pending_mentions.len(), 1);
    // A blank note has no text to repeat, so it is staged itself.
    assert!(state.send_annotation_to_agent(&NodeId::new("n2"), 0));
    assert!(state.chat.input.text().starts_with("@note"));
    assert_eq!(state.chat.pending_mentions[0].key, "n2");
}

#[test]
fn a_busy_chat_gets_a_fresh_tab_and_non_annotations_are_refused() {
    let mut state = state();
    state.chat.messages.push(ChatMessage::user("earlier"));
    let tabs = state.chat.tabs().len();
    assert!(state.send_annotation_to_agent(&NodeId::new("p1"), 0));
    assert_eq!(state.chat.tabs().len(), tabs + 1);
    assert!(state.chat.messages.is_empty());
    // Nothing around the prompt: only its text.
    assert_eq!(state.chat.input.text(), "Add a footer");
    assert!(!state.send_annotation_to_agent(&NodeId::new("hero"), 0));
    assert!(!state.send_annotation_to_agent(&NodeId::new("missing"), 0));
}

#[test]
fn a_note_inside_a_frame_is_placed_in_page_coordinates() {
    let doc = serde_json::json!({
        "version": "1.0.0",
        "children": [
            {"type": "frame", "id": "board", "name": "Board", "x": 1000, "y": 1000,
             "width": 800, "height": 600, "children": [
                {"type": "note", "id": "inner", "x": 10, "y": 10, "width": 100, "height": 80,
                 "content": "Tighten the spacing"}
             ]},
            {"type": "frame", "id": "origin", "name": "Origin", "x": 0, "y": 0, "width": 200, "height": 200}
        ]
    });
    let state = EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    let page = state.active_children();
    // Parent-relative (10, 10) would sit on Origin; in page space the note
    // is at (1010, 1010), on its own board.
    assert_eq!(
        ids(surrounding_layers(page, find(&state, "inner"))),
        ["board"]
    );
}
