use super::*;
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::json;

fn doc(value: Value) -> PenDocument {
    jian_ops_schema::load_str(&value.to_string())
        .expect("fixture parses")
        .value
}

/// A card with a header text and an empty content slot, a button with a
/// label, and a toolbar component that nests a button instance.
fn library() -> Vec<Value> {
    vec![
        json!({"type":"frame","id":"card","name":"Card","reusable":true,
               "width":200,"height":120,"layout":"vertical","children":[
            {"type":"text","id":"title","name":"Title","content":"Hello"},
            {"type":"frame","id":"slot","name":"Content","slot":[],
             "fill":[{"type":"solid","color":"#eeeeee"}],"children":[
                {"type":"text","id":"placeholder","content":"Drop here"}]}
        ]}),
        json!({"type":"frame","id":"button","name":"Button","reusable":true,
               "width":120,"height":40,"children":[
            {"type":"text","id":"label","name":"Label","content":"OK"}]}),
        json!({"type":"frame","id":"toolbar","name":"Toolbar","reusable":true,
               "width":300,"height":48,"children":[
            {"type":"ref","id":"save","ref":"button",
             "descendants":{"label":{"content":"Save"}}}]}),
    ]
}

fn resolved_with(instance: Value) -> PenNode {
    let mut children = library();
    children.push(instance);
    let resolved = resolve_refs_for_canvas(&doc(json!({"version":"1.0.0","children":children})));
    resolved.children.last().expect("instance").clone()
}

fn find<'a>(node: &'a PenNode, id: &str) -> &'a PenNode {
    crate::walkers::find_node(std::slice::from_ref(node), &crate::NodeId::new(id))
        .unwrap_or_else(|| panic!("{id} is in the expanded instance"))
}

fn text(node: &PenNode) -> &str {
    match node {
        PenNode::Text(text) => match &text.content {
            TextContent::Plain(content) => content,
            other => panic!("plain text expected, got {other:?}"),
        },
        other => panic!("text node expected, got {other:?}"),
    }
}

#[test]
fn a_typed_entry_replaces_the_whole_descendant() {
    let inst = resolved_with(json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "title":{"type":"rectangle","id":"bar","width":40,"height":4}}}));
    let bar = find(&inst, "inst__bar");
    let PenNode::Rectangle(rect) = bar else {
        panic!("the replacement's own type renders, got {bar:?}");
    };
    assert_eq!(rect.base.name, None, "no field of the master text survives");
    assert!(crate::walkers::find_node(
        std::slice::from_ref(&inst),
        &crate::NodeId::new("inst__title")
    )
    .is_none());
}

#[test]
fn a_replacement_without_an_id_keeps_the_slot_id() {
    let inst = resolved_with(json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "title":{"type":"text","content":"Swapped"}}}));
    assert_eq!(text(find(&inst, "inst__title")), "Swapped");
}

#[test]
fn a_replacement_subtree_gets_virtual_ids_and_flat_patches() {
    let inst = resolved_with(json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "slot":{"type":"frame","id":"custom","layout":"vertical","children":[
            {"type":"text","id":"item1","content":"One"},
            {"type":"text","id":"item2","content":"Two"}]},
        "item2":{"content":"Second"}}}));
    let custom = find(&inst, "inst__custom");
    assert_eq!(custom.children().map(Vec::len), Some(2));
    assert_eq!(text(find(&inst, "inst__item1")), "One");
    assert_eq!(text(find(&inst, "inst__item2")), "Second");
}

#[test]
fn a_children_patch_fills_a_slot_and_keeps_its_props() {
    let inst = resolved_with(json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "slot":{"children":[{"type":"text","id":"mine","content":"Mine"}]}}}));
    let slot = find(&inst, "inst__slot");
    assert_eq!(crate::fills::first_solid_fill_hex(slot), Some("#eeeeee"));
    let kids = slot.children().expect("slot keeps a child list");
    assert_eq!(kids.len(), 1, "the master's placeholder is replaced");
    assert_eq!(text(&kids[0]), "Mine");
    assert_eq!(kids[0].id_str(), "inst__mine");
}

#[test]
fn a_path_key_reaches_into_a_nested_instance() {
    let inst = resolved_with(
        json!({"type":"ref","id":"bar","ref":"toolbar","descendants":{
        "save/label":{"content":"Store"}}}),
    );
    assert_eq!(
        text(find(&inst, "bar__save__label")),
        "Store",
        "the outer path entry wins over the nested instance's own override"
    );
}

#[test]
fn a_path_key_replaces_inside_a_nested_instance() {
    let inst = resolved_with(
        json!({"type":"ref","id":"bar","ref":"toolbar","descendants":{
        "save/label":{"type":"icon_font","id":"glyph","iconFontName":"check",
                      "width":16,"height":16}}}),
    );
    assert!(matches!(
        find(&inst, "bar__save__glyph"),
        PenNode::IconFont(_)
    ));
}

#[test]
fn a_patch_on_a_nested_instance_styles_its_root() {
    let inst = resolved_with(
        json!({"type":"ref","id":"bar","ref":"toolbar","descendants":{
        "save":{"fill":[{"type":"solid","color":"#ff0000"}]}}}),
    );
    let save = find(&inst, "bar__save");
    assert!(matches!(save, PenNode::Frame(_)));
    assert_eq!(crate::fills::first_solid_fill_hex(save), Some("#ff0000"));
    assert_eq!(text(find(&inst, "bar__save__label")), "Save");
}

#[test]
fn a_plain_key_does_not_reach_into_a_nested_instance() {
    let inst = resolved_with(
        json!({"type":"ref","id":"bar","ref":"toolbar","descendants":{
        "label":{"content":"Nope"}}}),
    );
    assert_eq!(text(find(&inst, "bar__save__label")), "Save");
}

#[test]
fn replacements_survive_save_and_load_and_master_edits() {
    let mut children = library();
    children.push(json!({"type":"ref","id":"inst","ref":"card","descendants":{
        "slot":{"type":"frame","id":"custom","children":[
            {"type":"text","id":"item","content":"Kept"}]}}}));
    let loaded = doc(json!({"version":"1.0.0","children":children}));
    let saved = serde_json::to_string(&loaded).expect("serializes");
    let mut reloaded = jian_ops_schema::load_str(&saved).expect("reloads").value;
    // The master changes its title after the instance replaced the slot.
    if let Some(PenNode::Text(title)) =
        crate::walkers::find_node_mut(&mut reloaded.children, &crate::NodeId::new("title"))
    {
        title.content = TextContent::Plain("Renamed".into());
    }
    let resolved = resolve_refs_for_canvas(&reloaded);
    let inst = resolved.children.last().expect("instance");
    assert_eq!(text(find(inst, "inst__item")), "Kept");
    assert_eq!(text(find(inst, "inst__title")), "Renamed");
}

#[test]
fn detach_materializes_replacements_and_nested_paths() {
    let mut children = library();
    children.push(
        json!({"type":"ref","id":"bar","ref":"toolbar","descendants":{
        "save/label":{"content":"Store"}}}),
    );
    let document = doc(json!({"version":"1.0.0","children":children}));
    let reference = document.children.last().expect("instance").clone();
    let toolbar = document.children[2].clone();
    let detached =
        crate::ref_resolve::materialize_instance(&reference, &toolbar).expect("materializes");
    let PenNode::Ref(save) = &detached.children().expect("children")[0] else {
        panic!("the nested instance stays an instance");
    };
    let label = save
        .descendants
        .as_ref()
        .and_then(|d| d.get("label"))
        .expect("forwarded entry");
    assert_eq!(label["content"], "Store");
}

#[test]
fn owned_nodes_patch_in_place_and_reset_clears_paths() {
    let mut entry = json!({"type":"frame","id":"custom","children":[
        {"type":"text","id":"item","content":"One","name":"Item"}]});
    let patch = json!({"content":"Two","name":null});
    assert!(patch_owned_node(
        &mut entry,
        "slot",
        "item",
        patch.as_object().unwrap()
    ));
    assert_eq!(entry["children"][0]["content"], "Two");
    assert!(entry["children"][0].get("name").is_none());
    assert!(!patch_owned_node(
        &mut entry,
        "slot",
        "missing",
        patch.as_object().unwrap()
    ));

    let mut descendants = json!({"slot":{}, "slot/x":{}, "slotted":{}, "title":{}})
        .as_object()
        .unwrap()
        .clone();
    assert!(reset_entry(&mut descendants, "slot"));
    let keys: Vec<&str> = descendants.keys().map(String::as_str).collect();
    assert_eq!(keys, ["slotted", "title"]);
    assert!(!reset_entry(&mut descendants, "slot"));
}

#[test]
fn instance_props_on_a_nested_instance_win_over_its_authored_ones() {
    let document = doc(json!({"version":"1.0.0","children":[
        library()[1].clone(),
        {"type":"frame","id":"bar","reusable":true,"width":300,"height":48,"children":[
            {"type":"ref","id":"save","ref":"button","x":10,"y":4,"name":"Save",
             "descendants":{"label":{"type":"icon_font","id":"glyph","iconFontName":"check",
                                     "width":16,"height":16}}}]},
        {"type":"ref","id":"top","ref":"bar","descendants":{
            "save":{"x":50,"name":"Moved"},
            "save/label":{"fill":[{"type":"solid","color":"#ff0000"}]}}}
    ]}));
    let resolved = resolve_refs_for_canvas(&document);
    let inst = resolved.children.last().expect("instance");
    let save = find(inst, "top__save");
    assert_eq!(save.base().x, Some(50.0));
    assert_eq!(save.base().name.as_deref(), Some("Moved"));
    let glyph = find(inst, "top__save__glyph");
    assert!(
        matches!(glyph, PenNode::IconFont(_)),
        "an outer patch styles the master's replacement instead of undoing it"
    );
    assert_eq!(crate::fills::first_solid_fill_hex(glyph), Some("#ff0000"));
}
