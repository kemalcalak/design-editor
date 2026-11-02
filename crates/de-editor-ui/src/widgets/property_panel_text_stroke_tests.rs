//! The Stroke section for text (#282): text nodes are outlined, so the
//! panel offers their stroke like a shape's.

use super::property_panel::{PropertyPanel, SectionCapabilities};
use super::property_panel_test_support::state_from;
use de_editor_core::NodeId;

#[test]
fn a_selected_text_node_shows_its_stroke() {
    let mut state = state_from(
        r##"{"version":"1.0.0","children":[
            {"type":"text","id":"t","x":0,"y":0,"content":"Outlined","fontSize":32,
             "stroke":{"thickness":2,"align":"outside","fill":[{"type":"solid","color":"#FF00FF"}]}}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("t"));
    let panel = PropertyPanel::for_selection(&state).expect("text panel");
    assert!(SectionCapabilities::for_kind(&panel.snapshot.kind_variant).stroke);
    let stroke = panel.snapshot.stroke.expect("the stroke reaches the panel");
    assert_eq!(stroke.width, 2.0);
}
