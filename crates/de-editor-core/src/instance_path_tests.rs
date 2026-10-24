use super::*;
use crate::command::EditorCommand;

const DOC: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"frame","id":"badge","name":"Badge","reusable":true,"width":40,"height":20,
     "children":[{"type":"text","id":"label","name":"Label","content":"New"}]},
    {"type":"frame","id":"card","name":"Card","reusable":true,"x":0,"y":0,"width":200,"height":100,
     "children":[
       {"type":"text","id":"title","name":"Title","content":"Hello"},
       {"type":"ref","id":"pill","ref":"badge"},
       {"type":"frame","id":"grp","name":"Group","children":[
         {"type":"text","id":"grp__a","name":"A","content":"a"}
       ]}
     ]},
    {"type":"ref","id":"inst1","ref":"card","x":300,"y":50},
    {"type":"frame","id":"plain","name":"Plain","children":[
      {"type":"text","id":"plain__note","name":"Note","content":"authored"}
    ]}
  ]
}"##;

fn state() -> EditorState {
    let doc = jian_ops_schema::load_str(DOC)
        .expect("fixture parses")
        .value;
    EditorState::from_document(doc)
}

fn descendants(state: &EditorState) -> serde_json::Value {
    match walkers::find_node(state.active_children(), &NodeId::new("inst1")) {
        Some(PenNode::Ref(r)) => serde_json::to_value(&r.descendants).unwrap(),
        other => panic!("inst1 must stay a Ref, got {other:?}"),
    }
}

#[test]
fn anchor_needs_a_real_two_segment_instance_path() {
    let s = state();
    assert_eq!(
        instance_path_anchor(&s.doc, "inst1/title"),
        Some(NodeId::new("inst1__title"))
    );
    assert_eq!(instance_path_anchor(&s.doc, "inst1/missing"), None);
    assert_eq!(instance_path_anchor(&s.doc, "plain/plain__note"), None);
    assert_eq!(
        instance_path_anchor(&s.doc, "inst1/pill/label"),
        None,
        "nested instance children stay read-only"
    );
    assert_eq!(instance_path_anchor(&s.doc, "inst1/"), None);
}

#[test]
fn patch_on_an_instance_path_becomes_a_descendant_override() {
    let mut s = state();
    assert!(s.apply(EditorCommand::PatchNodeData {
        node_id: NodeId::new("inst1/title"),
        patch_json: r#"{"content":"Hi there"}"#.into(),
        page_id: None,
    }));
    assert_eq!(
        descendants(&s),
        serde_json::json!({"title": {"content": "Hi there"}})
    );
    // The component itself is untouched.
    let master = walkers::find_node(s.active_children(), &NodeId::new("title")).unwrap();
    assert_eq!(
        serde_json::to_value(master).unwrap()["content"],
        serde_json::json!("Hello")
    );
}

#[test]
fn update_on_an_instance_path_routes_the_name() {
    let mut s = state();
    assert!(s.apply(EditorCommand::UpdateNode {
        node_id: NodeId::new("inst1/title"),
        x: None,
        y: None,
        width: None,
        height: None,
        name: Some("Headline".into()),
        fill_hex: None,
        page_id: None,
    }));
    assert_eq!(
        descendants(&s),
        serde_json::json!({"title": {"name": "Headline"}})
    );
}

#[test]
fn structural_patch_on_an_instance_path_is_refused_and_leaves_the_ref() {
    let mut s = state();
    assert!(!s.apply(EditorCommand::PatchNodeData {
        node_id: NodeId::new("inst1/title"),
        patch_json: r#"{"id":"other"}"#.into(),
        page_id: None,
    }));
    assert_eq!(descendants(&s), serde_json::Value::Null);
}

#[test]
fn expanded_virtual_ids_become_paths() {
    let s = state();
    let inst = walkers::find_node(s.active_children(), &NodeId::new("inst1"))
        .unwrap()
        .clone();
    let plain = walkers::find_node(s.active_children(), &NodeId::new("plain"))
        .unwrap()
        .clone();
    let expanded = expand_instances_with_paths(&[inst, plain], &s.doc);
    let ids = |node: &PenNode| -> Vec<String> {
        fn walk(node: &PenNode, out: &mut Vec<String>) {
            out.push(node.id_str().to_string());
            for child in node.children().map(Vec::as_slice).unwrap_or_default() {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        walk(node, &mut out);
        out
    };
    assert_eq!(
        ids(&expanded[0]),
        [
            "inst1",
            "inst1/title",
            "inst1/pill",
            "inst1/pill/label",
            "inst1/grp",
            "inst1/grp__a"
        ],
        "a frame whose child ids extend its own is not taken for an instance"
    );
    assert_eq!(
        ids(&expanded[1]),
        ["plain", "plain__note"],
        "a node outside any instance keeps its authored id"
    );
}
