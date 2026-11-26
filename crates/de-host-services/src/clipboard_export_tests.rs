//! Copy as PNG / SVG (#399): what lands on the clipboard for one and for
//! several selected nodes.

use super::*;
use de_editor_core::NodeId;

fn state(selected: &[&str]) -> EditorState {
    let doc = jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"card","name":"Card","x":0,"y":0,"width":120,"height":80,
             "fill":[{"type":"solid","color":"#ffffff"}],
             "children":[
               {"type":"rectangle","id":"chip","name":"Chip","x":10,"y":10,"width":24,"height":24,
                "fill":[{"type":"solid","color":"#3366ff"}]}
             ]},
            {"type":"ellipse","id":"dot","name":"Dot","x":200,"y":0,"width":40,"height":40,
             "fill":[{"type":"solid","color":"#ff0000"}]}
        ]}"##,
    )
    .expect("fixture parses")
    .value;
    let mut s = EditorState::from_document(doc);
    s.selection.set = selected.iter().map(|id| NodeId::new(*id)).collect();
    if let Some(first) = selected.first() {
        s.selection.anchor = NodeId::new(*first);
    }
    s
}

/// PNG width / height from the IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG signature");
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
    (w, h)
}

#[test]
fn one_node_copies_as_a_tight_2x_png() {
    let png = selection_png(&state(&["card"])).expect("renders");
    assert_eq!(png_size(&png), (240, 160));
}

#[test]
fn several_nodes_copy_together_over_their_union() {
    // card 0..120 and dot 200..240 wide, both from y 0; 2x.
    let png = selection_png(&state(&["card", "dot"])).expect("renders");
    assert_eq!(png_size(&png), (480, 160));
}

#[test]
fn a_child_of_a_selected_frame_is_not_painted_twice() {
    let s = state(&["card", "chip"]);
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&s);
    let page = scene.active_page().unwrap();
    assert_eq!(topmost_selected(&s, page), vec!["card".to_string()]);
}

#[test]
fn svg_copy_carries_layer_names_and_geometry() {
    let svg = selection_svg(&state(&["card"])).expect("serializes");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("width=\"120\""), "{svg}");
    assert!(svg.contains("Chip"), "layer name as id: {svg}");
}

#[test]
fn multi_selection_svg_holds_every_node() {
    let svg = selection_svg(&state(&["card", "dot"])).expect("serializes");
    assert!(svg.contains("Card") && svg.contains("Dot"), "{svg}");
    assert!(svg.contains("width=\"240\""), "{svg}");
}

#[test]
fn an_empty_selection_is_a_typed_error() {
    assert!(matches!(
        selection_png(&state(&[])),
        Err(ClipboardExportError::NothingSelected)
    ));
    assert!(matches!(
        selection_svg(&state(&[])),
        Err(ClipboardExportError::NothingSelected)
    ));
}

#[test]
fn a_multi_copy_paints_in_layer_order_not_click_order() {
    // `dot` sits above `card`; picking it first must not put `card` on top.
    let s = state(&["dot", "card"]);
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&s);
    let page = scene.active_page().unwrap();
    assert_eq!(
        topmost_selected(&s, page),
        vec!["card".to_string(), "dot".to_string()]
    );
}
