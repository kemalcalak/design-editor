//! Golden tests for the HTML + CSS target, plus the shared walker rules.
//!
//! Goldens live in `tests/fixtures/html_export/`; regenerate them with
//! `UPDATE_GOLDEN=1 cargo test -p de-codegen flex::` and review the diff.

use super::*;

pub(super) fn load(src: &str) -> PenDocument {
    jian_ops_schema::load_str(src).expect("fixture parses").value
}

pub(super) fn fixture(name: &str) -> PenDocument {
    let path = format!(
        "{}/tests/fixtures/html_export/{name}.de",
        env!("CARGO_MANIFEST_DIR")
    );
    load(&std::fs::read_to_string(&path).expect("fixture file"))
}

/// Compare `actual` with the golden file, or rewrite it under UPDATE_GOLDEN.
pub(super) fn assert_golden(file: &str, actual: &str) {
    let path = format!(
        "{}/tests/fixtures/html_export/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        expected == actual,
        "golden {file} differs; rerun with UPDATE_GOLDEN=1 and review.\n--- actual ---\n{actual}"
    );
}

fn css(doc: &PenDocument) -> String {
    Html.generate(doc)
}

#[test]
fn card_matches_the_golden_html_css() {
    assert_golden("card.html", &css(&fixture("card")));
}

#[test]
fn stack_matches_the_golden_html_css() {
    assert_golden("stack.html", &css(&fixture("stack")));
}

#[test]
fn auto_layout_becomes_flex_and_only_overlays_are_absolute() {
    let html = css(&fixture("card"));
    assert!(html.contains("  display: flex;\n  flex-direction: column;\n  align-items: flex-start;\n  gap: 12px;\n  padding: 24px 20px;"), "{html}");
    // The overlay badge keeps its offset; the in-flow title does not get one.
    assert!(html.contains("position: absolute;\n  left: 292px;\n  top: 16px;"), "{html}");
    assert_eq!(html.matches("position: absolute").count(), 1, "{html}");
    // fill_container: cross axis stretches, main axis shares the row.
    assert!(html.contains("z-index: 3;\n  align-self: stretch;"), "{html}");
    // A fixed main-axis size never shrinks.
    assert!(html.contains(".swatch {\n  width: 40px;\n  height: 40px;\n  flex-shrink: 0;"), "{html}");
}

#[test]
fn variables_are_custom_properties_and_references_read_them() {
    let html = css(&fixture("card"));
    assert!(html.contains(":root {\n  --brand: #3366ff;\n  --space: 16;\n}"), "{html}");
    assert!(html.contains("color: var(--brand);"), "{html}");
    assert!(html.contains("gap: calc(var(--space) * 1px);"), "{html}");
}

#[test]
fn themed_variables_select_the_first_theme_value_by_default() {
    let html = css(&fixture("stack"));
    assert!(html.contains("<html lang=\"en\" data-Mode=\"Light\">"), "{html}");
    assert!(html.contains(":root[data-Mode=\"Light\"] {\n  --surface: #f8fafc;"), "{html}");
    assert!(html.contains("background: var(--surface);"), "{html}");
}

#[test]
fn text_links_become_anchors() {
    let html = css(&fixture("card"));
    assert!(
        html.contains("<a class=\"docs\" href=\"https://example.com/docs\">Read the docs</a>"),
        "{html}"
    );
}

#[test]
fn styled_segments_keep_their_style_and_links() {
    let doc = load(
        r##"{"version":"1.0","children":[{"type":"text","id":"t","name":"Note","content":[
          {"text":"See "},
          {"text":"terms","href":"https://example.com/terms","underline":true,"fill":"#2563eb"},
          {"text":" & more","fontWeight":700}
        ]}]}"##,
    );
    let html = css(&doc);
    assert!(
        html.contains("See <a href=\"https://example.com/terms\" class=\"note-run\">terms</a><span class=\"note-run-2\"> &amp; more</span>"),
        "{html}"
    );
    assert!(html.contains(".note-run {\n  color: rgb(37,99,235);\n  text-decoration: underline;\n}"), "{html}");
}

#[test]
fn stack_children_share_one_cell_and_the_first_child_is_on_top() {
    let html = css(&fixture("stack"));
    assert!(html.contains("  display: grid;\n  justify-items: center;\n  align-items: center;"), "{html}");
    assert!(html.contains(".headline {\n  grid-area: 1 / 1;\n  position: relative;\n  z-index: 3;"), "{html}");
    assert!(html.contains(".glow {\n  position: absolute;\n  left: 40px;\n  top: 20px;\n  z-index: 2;"), "{html}");
    assert!(html.contains(".backdrop {\n  grid-area: 1 / 1;\n  position: relative;\n  z-index: 1;\n  width: 100%;\n  height: 100%;"), "{html}");
}

#[test]
fn paint_maps_gradients_radii_blend_opacity_and_rotation() {
    let html = css(&fixture("stack"));
    assert!(html.contains("background: radial-gradient(circle 60px at 50% 50%, rgb(245,158,11) 0%, rgba(245,158,11,0) 100%);"), "{html}");
    assert!(html.contains("border-radius: 24px 0 24px 0;"), "{html}");
    assert!(html.contains("opacity: 0.8;"), "{html}");
    assert!(html.contains("mix-blend-mode: multiply;"), "{html}");
    assert!(html.contains("transform: rotate(15deg);"), "{html}");
    // The first fill is the top background layer.
    assert!(html.contains("background: linear-gradient(180deg, rgba(0,0,0,0) 0%, rgba(0,0,0,0.6) 100%), linear-gradient(rgb(124,58,237), rgb(124,58,237));"), "{html}");
    assert!(html.contains("text-shadow: 0 2px 4px rgba(0,0,0,0.4);"), "{html}");
}

#[test]
fn strokes_and_shadows_leave_the_layout_box_alone() {
    let html = css(&fixture("card"));
    assert!(html.contains("outline: 1px solid rgb(229,231,235);\n  outline-offset: -1px;"), "{html}");
    assert!(html.contains("box-shadow: 0 4px 12px 0 rgba(0,0,0,0.102);"), "{html}");
    assert!(!html.contains("border:"), "{html}");
}

#[test]
fn text_growth_controls_wrapping() {
    let html = css(&fixture("card"));
    assert!(html.contains("z-index: 4;\n  font-family: \"Helvetica\", sans-serif;"), "{html}");
    assert!(html.contains("white-space: pre;"), "{html}");
    assert!(html.contains("white-space: pre-wrap;"), "{html}");
    let doc = load(
        r#"{"version":"1.0","children":[{"type":"text","id":"t","content":"x","textGrowth":"auto","width":300,"height":40}]}"#,
    );
    assert!(!css(&doc).contains("width: 300px"), "auto text ignores numbers");
}

#[test]
fn layer_data_and_fragment_options() {
    let doc = fixture("card");
    let out = export_html(
        &doc,
        HtmlTarget::Css,
        &HtmlExportOptions {
            scaffold: false,
            layer_data: true,
            ..HtmlExportOptions::default()
        },
    );
    assert!(!out.html.contains("<!DOCTYPE html>"), "{}", out.html);
    assert!(out.html.contains("data-name=\"Card\" data-id=\"card\""), "{}", out.html);
}

#[test]
fn embedded_images_are_written_next_to_the_html() {
    let doc = load(
        r#"{"version":"1.0","children":[
          {"type":"image","id":"hero","name":"Hero Photo","width":200,"height":100,
           "src":"data:image/png;base64,iVBORw0KGgo="},
          {"type":"rectangle","id":"r","name":"Cover","width":50,"height":50,
           "fill":[{"type":"image","url":"data:image/png;base64,iVBORw0KGgo=","mode":"fit"}]}
        ]}"#,
    );
    let out = export_html(
        &doc,
        HtmlTarget::Css,
        &HtmlExportOptions {
            asset_dir: Some("assets".into()),
            ..HtmlExportOptions::default()
        },
    );
    assert_eq!(out.assets.len(), 1);
    assert_eq!(out.assets[0].path, "assets/hero-photo.png");
    assert!(out.html.contains("<img class=\"hero-photo\" src=\"assets/hero-photo.png\" alt=\"Hero Photo\">"), "{}", out.html);
    assert!(out.html.contains("background: url(\"assets/hero-photo.png\") center / contain no-repeat;"), "{}", out.html);
    assert!(out.html.contains("object-fit: cover;"), "{}", out.html);
    // The plain generator keeps the image inline.
    assert!(Html.generate(&doc).contains("src=\"data:image/png;base64,"));
}

#[test]
fn hover_and_load_motion_survive_the_flex_export() {
    let doc = load(
        r#"{"version":"1.0","children":[
          {"type":"rectangle","id":"btn","width":120,"height":40,
           "transition":{"durationMs":150,"easing":"standard","properties":["opacity"]},
           "animations":[{"trigger":"hover","keyframes":{"from":{"opacity":1},"to":{"opacity":0.8}},"durationMs":150}]}
        ]}"#,
    );
    let html = css(&doc);
    assert!(html.contains("data-de-motion-node=\"btn\""), "{html}");
    assert!(html.contains("[data-de-motion-node=\"btn\"]:hover {"), "{html}");
    assert!(html.contains("transition: opacity 150ms cubic-bezier(0.2,0,0,1);"), "{html}");
}

#[test]
fn vectors_are_inline_svg() {
    let doc = load(
        r##"{"version":"1.0","children":[
          {"type":"path","id":"p","name":"Tick","width":24,"height":24,"d":"M0 0 L10 10",
           "fill":[{"type":"solid","color":"#000000"}]},
          {"type":"polygon","id":"s","name":"Star","width":20,"height":20,"polygonCount":5,"innerRadius":0.5},
          {"type":"line","id":"l","name":"Rule","x2":100,"y2":0,
           "stroke":{"thickness":2,"fill":[{"type":"solid","color":"#ff0000"}]}}
        ]}"##,
    );
    let html = css(&doc);
    assert!(html.contains("<svg class=\"tick\" viewBox=\"0 0 10 10\" preserveAspectRatio=\"none\"><path d=\"M0 0 L10 10\" fill=\"rgb(0,0,0)\"/></svg>"), "{html}");
    assert!(html.contains("<polygon points=\"50,0 "), "{html}");
    assert!(html.contains("<line x1=\"0\" y1=\"0\" x2=\"100\" y2=\"0\" stroke=\"rgb(255,0,0)\" stroke-width=\"2\""), "{html}");
}
