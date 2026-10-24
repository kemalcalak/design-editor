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
