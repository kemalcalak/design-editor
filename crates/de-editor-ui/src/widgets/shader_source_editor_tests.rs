use de_editor_core::{NodeId, ShaderTarget};

use super::*;
use crate::widgets::property_panel_test_support::state_from;

fn open_editor() -> EditorState {
    let mut s = state_from(
        r##"{ "version": "1.0.0", "children": [
          {"type":"rectangle","id":"a","name":"A","x":0,"y":0,"width":40,"height":40,
           "fill":[{"type":"shader","sksl":"half4 main(float2 p) {\n    return half4(1);\n}"}]}
        ]}"##,
    );
    s.set_single_selection(NodeId::new("a"));
    assert!(s.open_shader_source_editor(ShaderTarget::Fill(0)));
    s
}

fn editor_layout() -> EditorLayout {
    layout(Rect::xywh(1000.0, 60.0, 272.0, 700.0), 1300.0, 800.0)
}

#[test]
fn the_editor_floats_left_of_the_card_inside_the_window() {
    let l = editor_layout();
    assert!(l.panel.origin.x + l.panel.size.x <= 1000.0);
    assert!(l.panel.origin.y + l.panel.size.y <= 800.0);
    assert!(l.panel.contains(l.apply.origin) && l.panel.contains(l.cancel.origin));
    // A narrow window keeps it on screen.
    let narrow = layout(Rect::xywh(100.0, 60.0, 272.0, 700.0), 400.0, 800.0);
    assert!(narrow.panel.origin.x >= 0.0);
}

#[test]
fn a_click_in_the_code_places_the_caret_and_apply_is_one_undo_step() {
    let mut s = open_editor();
    let l = editor_layout();
    // Line 2, column 4 (`return`).
    let point = Point2D::new(
        l.text_x + 4.0 * CHAR_W + 1.0,
        l.code.origin.y + 8.0 + LINE_H * 1.5,
    );
    assert_eq!(apply_press(&mut s, &l, point, false, 0), Some(true));
    let editor = s.editor_ui.shader_editing.editor.as_ref().unwrap();
    assert_eq!(editor.caret_line_col(), (1, 4));
    de_editor_core::shader_source_editor::text(&mut s, 'x', 0);
    let depth = s.history.past.len();
    let apply = Point2D::new(l.apply.origin.x + 4.0, l.apply.origin.y + 4.0);
    assert_eq!(apply_press(&mut s, &l, apply, false, 0), Some(true));
    assert_eq!(s.history.past.len(), depth + 1);
    let program = s.selected_shader_program(ShaderTarget::Fill(0)).unwrap();
    assert!(program.sksl.unwrap().contains("    xreturn"));
    // Outside: the press goes through and the editor gives up the keys.
    assert_eq!(
        apply_press(&mut s, &l, Point2D::new(1.0, 1.0), false, 0),
        None
    );
    assert!(!de_editor_core::shader_source_editor::owns_keyboard(&s));
    let close = Point2D::new(l.close.origin.x + 4.0, l.close.origin.y + 4.0);
    assert_eq!(apply_press(&mut s, &l, close, false, 0), Some(true));
    assert!(s.editor_ui.shader_editing.editor.is_none());
}
