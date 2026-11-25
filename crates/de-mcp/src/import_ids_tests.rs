use super::*;
use serde_json::json;

#[test]
fn colliding_ids_inside_instance_content_are_renamed_too() {
    let mut nodes: Vec<PenNode> = vec![serde_json::from_value(json!({
        "type":"ref","id":"inst","ref":"card","descendants":{
            "slot":{"children":[{"type":"text","id":"n5","content":"Mine"}]},
            "n5":{"content":"Patched"}}
    }))
    .expect("node")];
    let existing: HashSet<NodeId> = [NodeId::new("n5")].into_iter().collect();
    assert_eq!(rename_colliding_ids(&mut nodes, &existing), 1);
    let PenNode::Ref(instance) = &nodes[0] else {
        panic!("instance");
    };
    let descendants = instance.descendants.as_ref().expect("map");
    assert_eq!(
        descendants["slot"].pointer("/children/0/id"),
        Some(&json!("n5-2"))
    );
    assert!(descendants.contains_key("n5-2"), "{descendants:?}");
}
