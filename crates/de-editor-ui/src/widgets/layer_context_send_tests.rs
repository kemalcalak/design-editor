//! "Send to agent" (#398): the row shows only on a sticky note / context /
//! prompt annotation, and its dispatch opens the chat on it.

use de_editor_core::editor_ui_state::{LayerContextMenuState, LayerContextTarget};
use de_editor_core::{EditorState, NodeId};

use super::{LayerContextAction, LayerContextMenu};
use crate::widgets::layer_context_flow::{apply_layer_context_action, LayerContextStep};

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"hero","name":"Hero","x":0,"y":0,"width":400,"height":300},
          {"type":"note","id":"n1","name":"Review","x":380,"y":40,"width":240,"height":160,
           "content":"Tighten the spacing"}
        ]}"#,
    )
    .unwrap()
    .value;
    EditorState::from_document(doc)
}

fn offers_send(s: &EditorState, id: &str) -> bool {
    LayerContextMenu::for_state(
        s,
        LayerContextMenuState {
            target: LayerContextTarget::Layer(NodeId::new(id)),
            anchor_x: 0.0,
            anchor_y: 0.0,
            menu: Default::default(),
        },
    )
    .rows
    .iter()
    .any(|row| row.action == LayerContextAction::SendToAgent)
}

#[test]
fn the_row_shows_only_on_annotations() {
    let s = state();
    assert!(offers_send(&s, "n1"));
    assert!(!offers_send(&s, "hero"));
}

#[test]
fn the_row_opens_the_chat_on_the_note() {
    let mut s = state();
    let mut next = 1;
    let step = apply_layer_context_action(
        &mut s,
        &mut next,
        LayerContextAction::SendToAgent,
        LayerContextTarget::Layer(NodeId::new("n1")),
        0,
    );
    assert_eq!(step, LayerContextStep::Done);
    assert_eq!(s.chat.input.text(), "Tighten the spacing\n\n@Review @Hero");
    assert_eq!(s.chat.pending_mentions.len(), 2);
    assert!(s.chat.focused);
}
