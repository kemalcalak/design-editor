//! A boolean group reaches the scene as the computed path of its
//! children (#388): curves kept, children hidden, the group's own paint.

use crate::editor_state_to_active_page_layout_scene;
use de_editor_core::EditorState;
use jian_scene::layout_scene::NodeKind;

fn scene_of(json: &str) -> jian_scene::layout_scene::LayoutScene {
    let state = EditorState::from_document(jian_ops_schema::load_str(json).expect("parses").value);
    editor_state_to_active_page_layout_scene(&state)
}

const TWO_CIRCLES: &str = r##"{
  "version":"1.0.0",
  "children":[
    {"type":"boolean","id":"b","name":"Union","op":"union",
     "fill":[{"type":"solid","color":"#ff0000"}],
     "children":[
       {"type":"ellipse","id":"c1","x":0,"y":0,"width":100,"height":100},
       {"type":"ellipse","id":"c2","x":60,"y":0,"width":100,"height":100}
     ]}
  ]
}"##;

#[test]
fn a_boolean_group_paints_its_computed_curves() {
    let scene = scene_of(TWO_CIRCLES);
    let b = &scene.pages[scene.active_page_index].children[0];
    assert_eq!(b.kind, NodeKind::Path);
    let d = b.svg_path.as_deref().expect("computed d");
    let cubics = d.matches('C').count();
    assert!((1..=8).contains(&cubics), "{cubics} cubics in {d}");
    assert!(!d.contains("L"), "no polyline: {d}");
    assert!((b.bounds.size.x - 160.0).abs() < 0.01 && (b.bounds.size.y - 100.0).abs() < 0.01);
    assert!(b.fill.is_some(), "the group's own fill");
    assert_eq!(b.children.len(), 2, "children kept for selection");
    assert!(b.children.iter().all(|c| c.hidden), "but never painted");
}

#[test]
fn subtract_uses_the_bottom_child_as_base() {
    let json = TWO_CIRCLES.replace("\"union\"", "\"subtract\"");
    let scene = scene_of(&json);
    let b = &scene.pages[scene.active_page_index].children[0];
    assert_eq!(b.kind, NodeKind::Path);
    // c1 minus c2 keeps c1's left side only.
    assert!((b.bounds.origin.x - 0.0).abs() < 0.01);
    assert!(
        b.bounds.size.x < 81.0 && b.bounds.size.x > 60.0,
        "{:?}",
        b.bounds
    );
}

#[test]
fn an_empty_result_stays_a_group() {
    let json = r##"{"version":"1.0.0","children":[
      {"type":"boolean","id":"b","op":"intersect","children":[
        {"type":"rectangle","id":"r1","x":0,"y":0,"width":10,"height":10},
        {"type":"rectangle","id":"r2","x":50,"y":0,"width":10,"height":10}]}]}"##;
    let scene = scene_of(json);
    let b = &scene.pages[scene.active_page_index].children[0];
    assert_eq!(b.kind, NodeKind::Group);
}

#[test]
fn nested_boolean_groups_resolve_inside_out() {
    let json = r##"{"version":"1.0.0","children":[
      {"type":"boolean","id":"outer","op":"subtract","children":[
        {"type":"rectangle","id":"base","x":0,"y":0,"width":100,"height":100},
        {"type":"boolean","id":"inner","op":"union","children":[
          {"type":"rectangle","id":"a","x":10,"y":10,"width":20,"height":20},
          {"type":"rectangle","id":"b","x":20,"y":10,"width":20,"height":20}]}]}]}"##;
    let scene = scene_of(json);
    let outer = &scene.pages[scene.active_page_index].children[0];
    assert_eq!(outer.kind, NodeKind::Path);
    assert!(outer.even_odd_fill, "the hole needs even-odd");
    assert_eq!(outer.children[1].kind, NodeKind::Path);
}

#[test]
fn a_moved_group_offsets_its_children() {
    // A group's own x / y (set when it is dragged) moves its children,
    // so path commits and the codegen bake add it to the frame origin.
    let json = r##"{"version":"1.0.0","children":[
      {"type":"group","id":"g","x":100,"y":50,"children":[
        {"type":"rectangle","id":"r","x":10,"y":10,"width":20,"height":20}]},
      {"type":"boolean","id":"b","x":100,"y":50,"op":"union","children":[
        {"type":"rectangle","id":"r1","x":10,"y":10,"width":20,"height":20},
        {"type":"rectangle","id":"r2","x":20,"y":10,"width":20,"height":20}]}]}"##;
    let scene = scene_of(json);
    let page = &scene.pages[scene.active_page_index];
    let r = page.find("r").unwrap();
    let b = page.find("b").unwrap();
    assert_eq!(r.bounds.origin.x, 110.0, "group child {:?}", r.bounds);
    assert_eq!(b.bounds.origin.x, 110.0, "boolean {:?}", b.bounds);
}
