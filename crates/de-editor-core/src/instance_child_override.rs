//! Virtual component-instance child lookup for inspector reads/writes.
//!
//! Canvas expansion gives every rendered instance descendant a
//! render-only anchor (`refId__childId`, `outer__inner__leaf` for a node
//! of a nested instance). This module validates that id against the real
//! Ref + component tree (via [`crate::instance_anchor`]) and resolves the
//! effective node without teaching generic document walkers about virtual
//! nodes.

use crate::instance_anchor::{locate_anchor, InstanceDescendant};
use crate::instance_override::resolve_instance_display_node;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

/// Split a canvas instance-child anchor into its authored Ref id and the
/// descendant's id path relative to that Ref (`child`, `nested/leaf`, or
/// the id of a node in instance-owned content). A delimiter match is not
/// enough: the candidate must correspond to a resolvable Ref and a node the
/// instance really renders.
pub fn split_instance_child_anchor(anchor: &NodeId, doc: &PenDocument) -> Option<(NodeId, NodeId)> {
    let hit = locate_anchor(doc, anchor)?;
    Some((hit.ref_id, NodeId::new_opt(&hit.target.path())?))
}

/// The located instance descendant behind a canvas anchor, with the
/// override target an edit writes to.
pub fn instance_descendant_for_anchor(
    doc: &PenDocument,
    anchor: &NodeId,
) -> Option<InstanceDescendant> {
    locate_anchor(doc, anchor)
}

/// Resolve an authored Ref root or one of its canvas-only virtual
/// child anchors into the effective node shown by the inspector.
/// Child results keep the virtual anchor id while all other fields
/// come from the component child plus its `descendants` entries.
pub fn resolve_instance_display_node_for_anchor(
    doc: &PenDocument,
    anchor: &NodeId,
) -> Option<PenNode> {
    if let Some(node) = crate::instance_anchor::find_authored(doc, anchor.as_str()) {
        return matches!(node, PenNode::Ref(_))
            .then(|| resolve_instance_display_node(doc, node))
            .flatten();
    }
    let mut child = locate_anchor(doc, anchor)?.display;
    child.base_mut().id = anchor.as_str().to_string();
    Some(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walkers::{find_node, find_node_mut};
    use crate::EditorState;

    const DOC: &str = r##"{
      "version":"1.0.0",
      "children":[
        {"type":"frame","id":"button","name":"Button","reusable":true,
         "width":120,"height":40,"children":[
           {"type":"icon_font","id":"icon","name":"home","iconFontName":"home",
            "width":24,"height":24,"fill":[{"type":"solid","color":"#111111"}]}
         ]},
        {"type":"ref","id":"inst","ref":"button","x":200,"y":80}
      ]
    }"##;

    fn state() -> EditorState {
        let doc = jian_ops_schema::load_str(DOC).expect("fixture").value;
        EditorState::from_document(doc)
    }

    fn rich_color_state() -> EditorState {
        let doc = jian_ops_schema::load_str(
            r##"{
              "version":"1.0.0",
              "children":[
                {"type":"frame","id":"card","name":"Card","reusable":true,
                 "width":120,"height":80,"children":[
                   {"type":"rectangle","id":"paint","name":"Paint",
                    "width":80,"height":40,
                    "fill":[
                      {"type":"linear_gradient","angle":0,"stops":[
                        {"offset":0,"color":"#ffffff"},
                        {"offset":1,"color":"#00000080"}
                      ]},
                      {"type":"solid","color":"#123456"}
                    ],
                    "effects":[{"type":"shadow","offsetX":0,"offsetY":4,
                      "blur":8,"spread":0,"color":"#00000040"}]}
                 ]},
                {"type":"ref","id":"inst","ref":"card","x":200,"y":80}
              ]
            }"##,
        )
        .expect("rich color fixture")
        .value;
        EditorState::from_document(doc)
    }

    fn ref_node(state: &EditorState) -> &jian_ops_schema::node::RefNode {
        match find_node(state.active_children(), &NodeId::new("inst")) {
            Some(PenNode::Ref(reference)) => reference,
            other => panic!("inst must remain a Ref, got {other:?}"),
        }
    }

    #[test]
    fn picker_opens_edits_and_records_history_for_virtual_child() {
        let mut state = state();
        state.set_single_selection(NodeId::new("inst__icon"));
        assert!(state.open_color_picker(crate::ui_draft::ColorTarget::Fill, 0.0));
        assert!(state.color_picker_set_hsv(0.0, 1.0, 1.0));
        assert!(state.close_color_picker());
        assert_eq!(state.history.past.len(), 1);
        assert_eq!(
            ref_node(&state)
                .descendants
                .as_ref()
                .and_then(|d| d.get("icon"))
                .and_then(|v| v.pointer("/fill/0/color"))
                .and_then(serde_json::Value::as_str),
            Some("#ff0000")
        );
    }

    #[test]
    fn picker_routes_every_node_color_target_to_virtual_child_override() {
        let mut indexed = rich_color_state();
        indexed.set_single_selection(NodeId::new("inst__paint"));
        assert!(indexed.open_color_picker_for_fill(crate::ui_draft::ColorTarget::Fill, 1, 0.0));
        assert!(indexed.color_picker_set_hsv(0.0, 1.0, 1.0));
        assert_eq!(
            ref_node(&indexed)
                .descendants
                .as_ref()
                .and_then(|d| d.get("paint"))
                .and_then(|v| v.pointer("/fill/1/color"))
                .and_then(serde_json::Value::as_str),
            Some("#ff0000")
        );

        let mut gradient = rich_color_state();
        gradient.set_single_selection(NodeId::new("inst__paint"));
        assert!(gradient.open_color_picker(crate::ui_draft::ColorTarget::GradientStop(1), 0.0));
        assert!(gradient.color_picker_set_hsv(120.0, 1.0, 1.0));
        assert_eq!(
            ref_node(&gradient)
                .descendants
                .as_ref()
                .and_then(|d| d.get("paint"))
                .and_then(|v| v.pointer("/fill/0/stops/1/color"))
                .and_then(serde_json::Value::as_str),
            Some("#00ff0080")
        );

        let mut effect = rich_color_state();
        effect.set_single_selection(NodeId::new("inst__paint"));
        assert!(effect.open_color_picker(crate::ui_draft::ColorTarget::EffectColor(0), 0.0));
        effect.color_picker_focus_hex();
        effect.color_picker_hex_backspace(0);
        effect.color_picker_hex_char('f', 1);
        assert_eq!(
            ref_node(&effect)
                .descendants
                .as_ref()
                .and_then(|d| d.get("paint"))
                .and_then(|v| v.pointer("/effects/0/color"))
                .and_then(serde_json::Value::as_str),
            Some("#00000f40")
        );
    }

    #[test]
    fn split_accepts_ref_id_ending_in_underscore() {
        let mut state = state();
        let PenNode::Ref(reference) = find_node(state.active_children(), &NodeId::new("inst"))
            .expect("ref")
            .clone()
        else {
            panic!("expected ref");
        };
        let mut underscored = reference;
        underscored.base.id = "inst_".into();
        state.doc.children.push(PenNode::Ref(underscored));

        assert_eq!(
            split_instance_child_anchor(&NodeId::new("inst___icon"), &state.doc),
            Some((NodeId::new("inst_"), NodeId::new("icon")))
        );
    }

    #[test]
    fn direct_fill_opacity_write_routes_to_virtual_child_override() {
        let mut state = state();
        state.set_single_selection(NodeId::new("inst__icon"));
        assert!(state.set_selected_fill_opacity(0.25));
        assert_eq!(
            ref_node(&state)
                .descendants
                .as_ref()
                .and_then(|d| d.get("icon"))
                .and_then(|v| v.pointer("/fill/0/opacity"))
                .and_then(serde_json::Value::as_f64),
            Some(0.25)
        );
    }

    #[test]
    fn history_repair_handles_a_full_past_deque() {
        let mut state = state();
        for _ in 0..crate::HISTORY_CAP {
            state.commit_history();
        }
        let child = NodeId::new("inst__icon");
        state.set_single_selection(child.clone());
        let scope = state
            .begin_instance_write_for_anchor()
            .expect("virtual child scope");
        state.commit_history();
        assert!(state.set_selected_color(true, "#ff0000"));
        state.finish_instance_write(scope);

        assert_eq!(state.history.past.len(), crate::HISTORY_CAP);
        let newest = state.history.past.back().expect("newest history snapshot");
        assert!(matches!(
            newest.doc.snapshot_find_node(0, &NodeId::new("inst")),
            Some(PenNode::Ref(_))
        ));
        assert!(newest.doc.snapshot_find_node(0, &child).is_none());
    }

    #[test]
    fn authored_node_wins_over_a_colliding_virtual_id() {
        let mut state = state();
        let ordinary: PenNode = serde_json::from_value(serde_json::json!({
            "type":"rectangle", "id":"inst__icon", "name":"Authored",
            "width":40, "height":40,
            "fill":[{"type":"solid","color":"#222222"}]
        }))
        .expect("ordinary authored node");
        state.doc.children.push(ordinary);
        state.set_single_selection(NodeId::new("inst__icon"));

        assert!(
            resolve_instance_display_node_for_anchor(&state.doc, &NodeId::new("inst__icon"))
                .is_none()
        );
        assert!(state.set_selected_color(true, "#00ff00"));
        let ordinary = find_node(state.active_children(), &NodeId::new("inst__icon"))
            .expect("authored node remains selected");
        assert_eq!(
            crate::fills::first_solid_fill_hex(ordinary),
            Some("#00ff00")
        );
        assert!(ref_node(&state).descendants.is_none());

        let master_icon = find_node_mut(state.active_children_mut(), &NodeId::new("icon"))
            .expect("master icon remains untouched");
        assert_eq!(
            crate::fills::first_solid_fill_hex(master_icon),
            Some("#111111")
        );
    }
}
