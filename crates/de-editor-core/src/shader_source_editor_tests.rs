use jian_ops_schema::style::{PenFill, ShaderFillBody};

use super::*;
use crate::fills::node_fills_mut;
use crate::test_support::{rect, state_with};

fn editor_state(sksl: &str) -> EditorState {
    let mut node = rect("a", "A", 0.0, 0.0, 40.0, 40.0);
    *node_fills_mut(&mut node).expect("fills") = vec![PenFill::Shader(ShaderFillBody {
        preset: None,
        sksl: Some(sksl.into()),
        uniforms: None,
        explain: None,
        opacity: None,
        blend_mode: None,
    })];
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("a"));
    assert!(s.open_shader_source_editor(ShaderTarget::Fill(0)));
    s
}

fn draft(s: &EditorState) -> &str {
    s.editor_ui
        .shader_editing
        .editor
        .as_ref()
        .unwrap()
        .input
        .text()
}

#[test]
fn line_and_column_round_trip() {
    let text = "ab\n  cd\n\né";
    assert_eq!(line_col(text, 0), (0, 0));
    assert_eq!(line_col(text, 5), (1, 2));
    assert_eq!(byte_at(text, 1, 2), 5);
    assert_eq!(byte_at(text, 1, 99), 7, "clamped to the line end");
    assert_eq!(byte_at(text, 3, 1), text.len());
    assert_eq!(byte_at(text, 9, 0), text.len());
}

#[test]
fn typing_keeps_indent_and_moves_by_line() {
    let mut s = editor_state("void f() {\n    x;\n}");
    // Down to `    x;`, End-of-line column, newline keeps the indent.
    assert_eq!(move_line(&mut s, true, false, 0), Some(true));
    for _ in 0..6 {
        move_caret(&mut s, true, false, 0);
    }
    assert_eq!(newline(&mut s, 0), Some(true));
    text(&mut s, 'y', 0);
    assert_eq!(draft(&s), "void f() {\n    x;\n    y\n}");
    text(&mut s, '\t', 0);
    assert!(draft(&s).contains("    y    \n"));
    assert_eq!(backspace(&mut s, 0), Some(true));
    assert_eq!(paste(&mut s, "a\r\nb", 0), Some(true));
    assert!(draft(&s).contains("y   a\nb\n}"));
    assert!(s.shader_source_editor_dirty());
}

#[test]
fn apply_writes_the_source_and_close_drops_the_draft() {
    let mut s = editor_state("half4 main(float2 p) { return half4(1); }");
    select_all(&mut s, 0);
    paste(&mut s, "half4 main(float2 p) { return half4(0); }", 0);
    assert!(s.apply_shader_source_editor());
    assert!(!s.shader_source_editor_dirty());
    let program = s.selected_shader_program(ShaderTarget::Fill(0)).unwrap();
    assert_eq!(
        program.sksl,
        Some("half4 main(float2 p) { return half4(0); }")
    );
    assert!(s.editor_ui.shader_editing.editor.is_some(), "stays open");
    assert!(s.close_shader_source_editor());
    assert_eq!(text(&mut s, 'x', 0), None, "closed editors own no keys");
}

#[test]
fn the_caret_line_stays_on_screen() {
    let long: String = (0..40).map(|i| format!("// {i}\n")).collect();
    let mut s = editor_state(&long);
    for _ in 0..30 {
        move_line(&mut s, true, false, 0);
    }
    let editor = s.editor_ui.shader_editing.editor.as_ref().unwrap();
    assert_eq!(editor.caret_line_col().0, 30);
    assert_eq!(editor.scroll_line, 30 + 1 - VISIBLE_LINES);
    let mut editor = editor.clone();
    assert!(editor.scroll_by(-100));
    assert_eq!(editor.scroll_line, 0);
    assert!(editor.scroll_by(1000));
    assert_eq!(editor.scroll_line, editor.line_count() - VISIBLE_LINES);
}
