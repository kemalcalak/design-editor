//! Copy as PNG / SVG rows (#399): the row leaves a request for the
//! desktop and retargets the selection like Duplicate does.

use super::*;
use de_editor_core::NodeId;

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"rectangle","id":"a","name":"a","x":0,"y":0,"width":10,"height":10},
          {"type":"rectangle","id":"b","name":"b","x":20,"y":0,"width":10,"height":10}
        ]}"#,
    )
    .unwrap()
    .value;
    EditorState::from_document(doc)
}

#[test]
fn copy_rows_queue_the_format_for_the_desktop() {
    let mut s = state();
    let mut next = 1;
    let step = apply_layer_context_action(
        &mut s,
        &mut next,
        LayerContextAction::CopyAsSvg,
        LayerContextTarget::Layer(NodeId::new("a")),
        0,
    );
    assert_eq!(step, LayerContextStep::Done);
    assert_eq!(
        s.editor_ui.canvas_view.pending_copy_as,
        Some(CopyAsFormat::Svg)
    );
    assert!(s.is_selected(&NodeId::new("a")));
}

#[test]
fn a_row_inside_the_multi_selection_keeps_it() {
    let mut s = state();
    s.selection.set = vec![NodeId::new("a"), NodeId::new("b")];
    s.selection.anchor = NodeId::new("a");
    let mut next = 1;
    apply_layer_context_action(
        &mut s,
        &mut next,
        LayerContextAction::CopyAsPng,
        LayerContextTarget::Layer(NodeId::new("b")),
        0,
    );
    assert_eq!(s.selection.set.len(), 2);
    assert_eq!(
        s.editor_ui.canvas_view.pending_copy_as,
        Some(CopyAsFormat::Png)
    );
}
