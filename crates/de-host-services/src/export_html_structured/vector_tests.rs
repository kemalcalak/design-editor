//! Icon-family and weight tests for the inline-SVG icon emitter (#4).

use super::emit_icon;
use de_editor_ui::layout_scene::{NodeKind, SceneNode};
use de_editor_ui::{Color, Rect};

fn icon_svg(family: &str, name: &str, weight: u16) -> String {
    let mut node = SceneNode::leaf("icon", NodeKind::Other("icon_font".into()));
    node.font_family = family.to_string();
    node.text = Some(name.to_string());
    node.font_weight = weight;
    node.fill = Some(Color::RED);
    let mut out = String::new();
    emit_icon(&mut out, &node, Rect::xywh(0.0, 0.0, 24.0, 24.0));
    out
}

#[test]
fn a_material_symbol_exports_as_a_filled_24_unit_glyph() {
    let svg = icon_svg("Material Symbols Rounded", "home", 0);
    assert!(svg.contains(r#"viewBox="0 0 24 24""#), "{svg}");
    assert!(
        svg.contains(r#"fill="rgb(255,0,0)" stroke="none""#),
        "{svg}"
    );
}

#[test]
fn a_heavy_material_weight_exports_the_canvas_outline() {
    let svg = icon_svg("material-symbols-outlined", "home", 700);
    assert!(
        svg.contains(
            r#"fill="rgb(255,0,0)" stroke="rgb(255,0,0)" stroke-width="1" stroke-linejoin="round""#
        ),
        "{svg}"
    );
}

#[test]
fn a_phosphor_weight_exports_the_weighted_glyph() {
    let regular = icon_svg("phosphor", "house", 400);
    let bold = icon_svg("ph", "house", 700);
    assert!(bold.contains(r#"viewBox="0 0 256 256""#), "{bold}");
    assert_ne!(regular, bold, "700 must pick house-bold");
    assert_eq!(bold, icon_svg("phosphor", "house-bold", 0));
}

/// The glyph `d` an exported `.de` board draws for one phosphor
/// `icon_font` carrying the extra JSON fields in `extra`.
fn doc_icon_glyph(name: &str, extra: serde_json::Value) -> String {
    let mut icon = serde_json::json!({
        "type": "icon_font", "id": "ic", "iconFontName": name,
        "iconFontFamily": "phosphor", "width": 24, "height": 24,
    });
    for (key, value) in extra.as_object().expect("extra fields") {
        icon[key] = value.clone();
    }
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{ "type": "frame", "id": "board", "width": 48, "height": 48,
                       "children": [icon] }],
    }))
    .expect("doc");
    let state = de_editor_core::EditorState::from_document(doc);
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    let body = super::super::board_slide_markup(&scene.pages[0], "board", "Board".into())
        .expect("board markup")
        .body;
    let start = body.find("<path d=\"").expect("icon path") + "<path d=\"".len();
    let len = body[start..].find('"').expect("path end");
    body[start..start + len].to_string()
}

#[test]
fn a_document_icon_weight_reaches_the_export() {
    let bold = doc_icon_glyph("house-bold", serde_json::json!({}));
    let numeric = doc_icon_glyph("house", serde_json::json!({ "weight": 700 }));
    let keyword = doc_icon_glyph("house", serde_json::json!({ "weight": "bold" }));
    assert_eq!(numeric, bold);
    assert_eq!(keyword, bold);
    assert_ne!(doc_icon_glyph("house", serde_json::json!({})), bold);
}
