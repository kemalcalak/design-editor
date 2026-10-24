//! Post-write repairs touch only what the write created or changed (#107).

use super::super::{load_editor_state, process_message, save_editor_state};
use super::*;
use de_editor_core::PenNodeExt;
use jian_ops_schema::node::PenNode;

/// A hand-made phone board the repairs would rewrite if they saw it: no
/// status bar, a fit-content height, an icon-shaped path and an empty image
/// slot. `n9` is an unrelated desktop board next to it.
const HAND_MADE: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"n1","name":"Phone","x":0,"y":0,"width":360,"height":"fit_content","layout":"vertical",
   "children":[
     {"type":"text","id":"n2","name":"Title","content":"Hello","fill":"#000000"},
     {"type":"path","id":"n3","name":"Chevron","d":"M6 9l6 6 6-6","width":14,"height":14,
      "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#111111"}]}},
     {"type":"rectangle","id":"n4","name":"Cover","width":240,"height":160,"fill":[{"type":"image","url":""}]}
   ]},
  {"type":"frame","id":"n9","name":"Desktop","x":800,"y":0,"width":1440,"height":900}
]}"##;

struct Doc {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl Doc {
    fn seed(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "design-editor-mcp-write-scope-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("doc.de");
        std::fs::write(&path, HAND_MADE).expect("seed document");
        // Round-trip once so every comparison below starts from the saved
        // shape (the writer spells empty containers as `"children": []`).
        let state = load_editor_state(&path).expect("load seed");
        save_editor_state(&state, &path).expect("save seed");
        Self { dir, path }
    }

    fn call(&self, tool: &str, arguments: serde_json::Value) {
        let mut state = load_editor_state(&self.path).expect("load document");
        let line = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": tool, "arguments": arguments}
        })
        .to_string();
        let response = process_message(&mut state, &self.path, &line)
            .expect("dispatch")
            .expect("response");
        assert!(!response.contains(r#""isError":true"#), "{response}");
    }

    fn root(&self, id: &str) -> PenNode {
        load_editor_state(&self.path)
            .expect("saved document")
            .active_children()
            .iter()
            .find(|node| node.id_str() == id)
            .cloned()
            .expect("root")
    }
}

impl Drop for Doc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn json(node: &PenNode) -> serde_json::Value {
    serde_json::to_value(node).expect("serialize node")
}

fn child<'a>(root: &'a PenNode, id: &str) -> &'a PenNode {
    root.children()
        .expect("children")
        .iter()
        .find(|node| node.id_str() == id)
        .expect("child")
}

#[test]
fn an_unrelated_write_leaves_a_hand_made_phone_board_unchanged() {
    let doc = Doc::seed("unrelated");
    let before = json(&doc.root("n1"));

    doc.call(
        "set_node_name",
        serde_json::json!({"node_id": "n9", "name": "Desktop renamed"}),
    );

    assert_eq!(json(&doc.root("n1")), before);
    assert_eq!(
        doc.root("n9").base().name.as_deref(),
        Some("Desktop renamed")
    );
}

#[test]
fn renaming_a_node_inside_the_board_repairs_nothing_else() {
    let doc = Doc::seed("rename-inside");
    let mut expected = json(&doc.root("n1"));
    expected["children"][0]["name"] = "Greeting".into();

    doc.call(
        "set_node_name",
        serde_json::json!({"node_id": "n2", "name": "Greeting"}),
    );

    assert_eq!(json(&doc.root("n1")), expected);
}

#[test]
fn a_write_into_the_board_repairs_only_the_nodes_it_added() {
    let doc = Doc::seed("insert-inside");
    let operations = r##"I("n1",{"type":"path","name":"New Chevron","d":"M6 9l6 6 6-6","width":14,"height":14,"stroke":{"thickness":2,"fill":[{"type":"solid","color":"#111111"}]}})"##;

    doc.call(
        "batch_design",
        serde_json::json!({"operations": operations}),
    );

    let phone = doc.root("n1");
    let children = phone.children().expect("children");
    assert!(
        !children
            .iter()
            .any(|node| node.base().role.as_deref() == Some("status-bar")),
        "a board the write did not create gets no status bar"
    );
    assert!(matches!(child(&phone, "n3"), PenNode::Path(_)));
    assert!(matches!(child(&phone, "n4"), PenNode::Rectangle(_)));
    let added = children
        .iter()
        .find(|node| node.base().name.as_deref() == Some("New Chevron"))
        .expect("added icon");
    assert!(
        matches!(added, PenNode::IconFont(_)),
        "the added icon path is normalized: {added:?}"
    );
}

#[test]
fn a_phone_screen_the_write_creates_is_repaired() {
    let doc = Doc::seed("new-screen");
    let operations = r##"I(null,{"type":"frame","name":"Generated","width":375,"height":"fit_content","layout":"vertical","children":[{"type":"rectangle","name":"Slot","width":240,"height":160,"fill":[{"type":"image","url":""}]}]})"##;

    doc.call(
        "batch_design",
        serde_json::json!({"operations": operations}),
    );

    let saved = load_editor_state(&doc.path).expect("saved document");
    let generated = saved
        .active_children()
        .iter()
        .find(|node| node.base().name.as_deref() == Some("Generated"))
        .expect("generated screen");
    let children = generated.children().expect("children");
    assert_eq!(children[0].base().role.as_deref(), Some("status-bar"));
    assert_eq!(generated.height_px(), Some(812.0));
    assert!(children
        .iter()
        .any(|node| matches!(node, PenNode::Image(_))));
    assert!(matches!(
        child(&doc.root("n1"), "n4"),
        PenNode::Rectangle(_)
    ));
}

#[test]
fn scope_classifies_new_extended_and_changed_nodes() {
    let doc = Doc::seed("scope");
    let mut state = load_editor_state(&doc.path).expect("load document");
    let baseline = WriteBaseline::capture(&state);

    let extra: PenNode = serde_json::from_value(serde_json::json!(
        {"type":"rectangle","id":"n5","name":"Added","width":10,"height":10}
    ))
    .expect("node");
    let new_root: PenNode = serde_json::from_value(serde_json::json!(
        {"type":"frame","id":"n6","name":"New","width":375,"height":800}
    ))
    .expect("node");
    let roots = state.active_children_mut();
    roots[0].children_mut().expect("children").push(extra);
    roots[1].base_mut().name = Some("Desktop renamed".to_string());
    roots.push(new_root);

    let scope = baseline.scope_after(&state);
    assert_eq!(scope.new_roots, vec!["n6".to_string()]);
    assert_eq!(scope.extended_roots, vec!["n1".to_string()]);
    let mut touched: Vec<&str> = scope.touched.iter().map(String::as_str).collect();
    touched.sort_unstable();
    assert_eq!(touched, vec!["n5", "n6", "n9"]);
}
