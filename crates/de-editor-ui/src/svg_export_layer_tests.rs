//! The layered SVG export (#407): layer tree, names, live text, embedded
//! images, effects and the warnings list, plus one golden file.

use super::*;
use crate::layout_scene::{
    BlurEffect, DropShadow, Effect, SceneStroke, SceneStrokeAlign, SceneTextAlign, SceneTextRun,
};
use crate::{ImageBlendMode, Point2D};

/// 1×1 transparent PNG.
const PIXEL_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

fn page(children: Vec<SceneNode>) -> ScenePage {
    ScenePage {
        id: "p1".into(),
        name: "Page 1".into(),
        children,
    }
}

fn frame(id: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, NodeKind::Frame);
    n.bounds = Rect::xywh(x, y, w, h);
    n
}

fn text(id: &str, content: &str, x: f32, y: f32) -> SceneNode {
    let mut n = SceneNode::leaf(id, NodeKind::Text);
    n.bounds = Rect::xywh(x, y, 200.0, 30.0);
    n.text = Some(content.into());
    n.font_family = "Inter".into();
    n.font_size = 24.0;
    n.font_weight = 600;
    n.fill = Some(Color::BLACK);
    n
}

fn photo(id: &str, src: &str) -> SceneNode {
    // Image nodes reach the scene as `rect` with an `image_src`.
    let mut n = SceneNode::leaf(id, NodeKind::Rect);
    n.bounds = Rect::xywh(16.0, 60.0, 120.0, 80.0);
    n.image_src = Some(src.into());
    n.corner_radius = 8.0;
    n
}

fn names(pairs: &[(&str, &str)]) -> LayerNames {
    pairs
        .iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}

fn layered(page: &ScenePage, node_id: &str, names: &LayerNames) -> SvgExport {
    let options = SvgExportOptions {
        layer_names: Some(names),
        embed_images: true,
    };
    serialize_node_on_page_svg_with(page, node_id, &options).expect("svg")
}

fn position(body: &str, needle: &str) -> usize {
    body.find(needle)
        .unwrap_or_else(|| panic!("missing {needle:?} in {body}"))
}

/// A card: auto-layout frame > header frame > two texts, an image, a group.
fn card() -> SceneNode {
    let mut card = frame("1", 0.0, 0.0, 320.0, 200.0);
    card.fill = Some(Color::WHITE);
    card.corner_radius = 16.0;
    card.clip_content = true;
    card.effects = vec![Effect::DropShadow(DropShadow {
        offset_x: 0.0,
        offset_y: 4.0,
        blur: 12.0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.25,
        },
        inner: false,
        blend_mode: ImageBlendMode::Normal,
    })];
    let mut header = frame("2", 16.0, 16.0, 288.0, 40.0);
    let title = text("3", "Pricing", 16.0, 16.0);
    let mut subtitle = text("4", "Per seat", 160.0, 16.0);
    subtitle.font_weight = 400;
    subtitle.font_size = 14.0;
    header.children = vec![title, subtitle];
    let mut dot = SceneNode::leaf("7", NodeKind::Ellipse);
    dot.bounds = Rect::xywh(280.0, 160.0, 16.0, 16.0);
    dot.fill = Some(Color::RED);
    let mut badges = SceneNode::leaf("6", NodeKind::Group);
    badges.bounds = dot.bounds;
    badges.children = vec![dot];
    card.children = vec![header, photo("5", PIXEL_PNG), badges];
    card
}

fn card_names() -> LayerNames {
    names(&[
        ("1", "Pricing card"),
        ("2", "Header"),
        ("3", "Title"),
        ("4", "Subtitle"),
        ("5", "Photo"),
        ("6", "Badges"),
        ("7", "Dot"),
    ])
}

#[test]
fn layer_names_become_unique_ids_with_data_names() {
    let mut a = frame("a", 0.0, 0.0, 10.0, 10.0);
    a.fill = Some(Color::RED);
    let mut b = frame("b", 20.0, 0.0, 10.0, 10.0);
    b.fill = Some(Color::RED);
    let mut c = frame("c", 40.0, 0.0, 10.0, 10.0);
    c.fill = Some(Color::RED);
    let p = page(vec![a, b, c]);
    let names = names(&[("a", "Hero card"), ("b", "Hero card")]);
    let options = SvgExportOptions {
        layer_names: Some(&names),
        embed_images: false,
    };
    let body = serialize_page_svg_with(&p, &options).expect("svg").svg;
    assert!(
        body.contains(r#"id="Hero card" data-name="Hero card""#),
        "{body}"
    );
    assert!(
        body.contains(r#"id="Hero card_2" data-name="Hero card""#),
        "{body}"
    );
    // A node without a name keeps its node id and gets no data-name.
    assert!(body.contains(r#"<rect id="c" "#), "{body}");
}

#[test]
fn nested_auto_layout_frames_and_groups_are_nested_g_layers() {
    let p = page(vec![card()]);
    let body = layered(&p, "1", &card_names()).svg;
    let card = position(&body, r#"<g id="Pricing card" data-name="Pricing card">"#);
    let header = position(&body, r#"<g id="Header" data-name="Header">"#);
    let title = position(&body, r#"<text id="Title""#);
    let subtitle = position(&body, r#"<text id="Subtitle""#);
    let badges = position(&body, r#"<g id="Badges" data-name="Badges">"#);
    let dot = position(&body, r#"<ellipse id="Dot""#);
    assert!(
        card < header && header < title && title < subtitle,
        "{body}"
    );
    assert!(subtitle < badges && badges < dot, "{body}");
    // The header closes before the image layer starts: siblings, not nesting.
    let header_end = header + body[header..].find("</g>").expect("header end");
    assert!(
        header_end < position(&body, r#"<image id="Photo""#),
        "{body}"
    );
    // The card's children sit in its content clip, its background outside it.
    let background = position(&body, r#"fill="rgb(255,255,255)""#);
    let clip = position(&body, r#"<g clip-path="url(#clip-n-31)">"#);
    assert!(
        card < background && background < clip && clip < header,
        "{body}"
    );
}

#[test]
fn text_stays_live_text_with_fonts_named() {
    let mut multi = text("t", "First line\nSecond & last", 0.0, 0.0);
    multi.text_align = SceneTextAlign::Center;
    multi.letter_spacing = 0.5;
    multi.text_runs = vec![SceneTextRun {
        start: 0,
        end: 5,
        font_size: 0.0,
        font_weight: 800,
        fill: Some(Color::RED),
        italic: false,
        underline: false,
        strikethrough: false,
    }];
    let p = page(vec![multi]);
    let body = layered(&p, "t", &names(&[("t", "Headline")])).svg;
    assert_eq!(body.matches("<text ").count(), 1, "one text layer: {body}");
    assert!(body.contains(r#"font-family="Inter""#), "{body}");
    assert!(body.contains(r#"font-size="24""#), "{body}");
    assert!(body.contains(r#"font-weight="600""#), "{body}");
    assert!(body.contains(r#"text-anchor="middle""#), "{body}");
    assert!(body.contains(r#"letter-spacing="0.5""#), "{body}");
    assert!(body.contains(r#"<tspan x="100" y=""#), "centred x: {body}");
    assert!(
        body.contains(r#"<tspan font-weight="800" fill="rgb(255,0,0)">First</tspan> line</tspan>"#),
        "styled run: {body}"
    );
    assert!(body.contains(">Second &amp; last</tspan>"), "{body}");
}

#[test]
fn images_are_embedded_and_clipped_to_their_corners() {
    let p = page(vec![card()]);
    let export = layered(&p, "1", &card_names());
    let body = &export.svg;
    assert!(
        body.contains(&format!(r#"<image id="Photo" data-name="Photo" x="16" y="60" width="120" height="80" xlink:href="{PIXEL_PNG}""#)),
        "{body}"
    );
    assert!(body.contains(r#"clip-path="url(#clip-n-35)""#), "{body}");
    assert!(body.contains(r#"<clipPath id="clip-n-35">"#), "{body}");
}

#[test]
fn a_linked_image_without_bytes_keeps_its_link_and_is_reported() {
    let p = page(vec![photo("5", "https://images.invalid/never-fetched.png")]);
    let export = layered(&p, "5", &names(&[("5", "Photo")]));
    assert!(
        export
            .svg
            .contains(r#"xlink:href="https://images.invalid/never-fetched.png""#),
        "{}",
        export.svg
    );
    assert_eq!(
        export.warnings,
        vec![
            "Photo (5): image is linked, not embedded (its bytes are not loaded): https://images.invalid/never-fetched.png"
                .to_string()
        ]
    );
}

#[test]
fn shadows_and_layer_blur_are_svg_filters() {
    let mut card = card();
    card.children.clear();
    card.effects.push(Effect::Blur(BlurEffect { radius: 6.0 }));
    let p = page(vec![card]);
    let body = layered(&p, "1", &card_names()).svg;
    assert!(body.contains(r#"<filter id="filter-n-31""#), "{body}");
    assert!(
        body.contains(r#"<feOffset dx="0" dy="4"/><feGaussianBlur stdDeviation="6"/>"#),
        "{body}"
    );
    assert!(
        body.contains(r#"values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.25 0""#),
        "{body}"
    );
    assert!(
        body.contains(r#"stdDeviation="3" result="effect2_foregroundBlur""#),
        "{body}"
    );
    assert!(body.contains(r#"filter="url(#filter-n-31)""#), "{body}");
}

#[test]
fn what_svg_cannot_carry_is_listed_in_warnings() {
    let mut glass = frame("g", 0.0, 0.0, 100.0, 100.0);
    glass.fill = Some(Color::WHITE);
    glass.effects = vec![Effect::BackgroundBlur { radius: 20.0 }];
    let mut mask = SceneNode::leaf("m", NodeKind::Ellipse);
    mask.bounds = Rect::xywh(0.0, 0.0, 50.0, 50.0);
    mask.fill = Some(Color::BLACK);
    mask.is_mask = true;
    let mut wrapped = text("w", "A line far too long for its narrow box", 0.0, 60.0);
    wrapped.bounds.size = Point2D::new(80.0, 30.0);
    wrapped.text_wrap = true;
    let mut outlined = frame("o", 0.0, 0.0, 40.0, 40.0);
    outlined.fill = Some(Color::WHITE);
    outlined.corner_radii = Some([4.0, 4.0, 0.0, 0.0]);
    glass.children = vec![mask, wrapped, outlined];
    let p = page(vec![glass]);
    let export = layered(&p, "g", &names(&[("g", "Glass"), ("w", "Body")]));
    let warnings = export.warnings.join("\n");
    assert!(
        warnings.contains("Glass (g): background blur"),
        "{warnings}"
    );
    assert!(
        warnings.contains("m: mask is exported as a plain shape"),
        "{warnings}"
    );
    assert!(
        warnings.contains("Body (w): text wraps at its box width"),
        "{warnings}"
    );
    assert!(warnings.contains("o: per-corner radii"), "{warnings}");
}

#[test]
fn an_inside_stroke_is_inset_so_it_stays_in_the_box() {
    let mut rect = frame("r", 0.0, 0.0, 100.0, 50.0);
    rect.fill = Some(Color::WHITE);
    rect.corner_radius = 10.0;
    rect.stroke = Some(SceneStroke {
        color: Color::BLACK,
        width: 4.0,
        sides: None,
        align: SceneStrokeAlign::Inside,
        style: Default::default(),
    });
    let body =
        serialize_node_on_page_svg_with(&page(vec![rect]), "r", &SvgExportOptions::default())
            .expect("svg")
            .svg;
    assert!(
        body.contains(r#"x="2" y="2" width="96" height="46" rx="8""#),
        "{body}"
    );
}

#[test]
fn hidden_and_canvas_only_layers_are_left_out() {
    let mut root = frame("root", 0.0, 0.0, 100.0, 100.0);
    root.fill = Some(Color::WHITE);
    let mut hidden = frame("hidden", 0.0, 0.0, 10.0, 10.0);
    hidden.fill = Some(Color::RED);
    hidden.hidden = true;
    let mut note = SceneNode::leaf("note", NodeKind::Other("note".into()));
    note.bounds = Rect::xywh(0.0, 0.0, 10.0, 10.0);
    note.fill = Some(Color::RED);
    root.children = vec![hidden, note];
    let body = layered(&page(vec![root]), "root", &LayerNames::new()).svg;
    assert!(!body.contains("hidden"), "{body}");
    assert!(!body.contains(r#"id="note""#), "{body}");
}

/// The whole file for the card, byte for byte. Regenerate with
/// `DE_UPDATE_GOLDEN=1 cargo test -p de-editor-ui svg_export` after an
/// intended change, and look at the diff.
#[test]
fn layered_card_matches_the_golden_file() {
    let p = page(vec![card()]);
    let export = layered(&p, "1", &card_names());
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/svg_export/testdata/layered_card.svg"
    );
    if std::env::var_os("DE_UPDATE_GOLDEN").is_some() {
        std::fs::write(golden_path, format!("{}\n", export.svg)).expect("write golden");
    }
    let golden = std::fs::read_to_string(golden_path).expect("golden file");
    assert_eq!(export.svg, golden.trim_end(), "golden mismatch");
    assert!(export.warnings.is_empty(), "{:?}", export.warnings);
}

#[test]
fn an_icon_is_a_named_vector_path_not_a_filled_box() {
    let mut icon = SceneNode::leaf("i", NodeKind::Other("icon_font".into()));
    icon.bounds = Rect::xywh(0.0, 0.0, 24.0, 24.0);
    icon.text = Some("check".into());
    icon.font_family = "lucide".into();
    icon.fill = Some(Color::BLACK);
    let export = layered(&page(vec![icon]), "i", &names(&[("i", "Done icon")]));
    let body = &export.svg;
    assert!(
        body.contains(r#"<path id="Done icon" data-name="Done icon""#),
        "{body}"
    );
    assert!(body.contains(r#"stroke="rgb(0,0,0)""#), "{body}");
    assert!(!body.contains("<rect"), "no box for an icon: {body}");
    assert!(export.warnings.is_empty(), "{:?}", export.warnings);
}

#[test]
fn a_linked_image_the_editor_has_loaded_is_embedded_once() {
    let url = "https://images.invalid/cached-photo.png";
    let png = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        PIXEL_PNG.trim_start_matches("data:image/png;base64,"),
    )
    .expect("fixture png");
    crate::widgets::canvas_viewport_image::store_remote_image_bytes(
        crate::layout_scene::stable_image_source_id(url),
        png,
    );
    let mut root = frame("root", 0.0, 0.0, 300.0, 200.0);
    root.children = vec![photo("a", url), photo("b", url)];
    let export = layered(&page(vec![root]), "root", &LayerNames::new());
    assert_eq!(
        export
            .svg
            .matches("xlink:href=\"data:image/png;base64,")
            .count(),
        2,
        "{}",
        export.svg
    );
    assert!(!export.svg.contains(url), "{}", export.svg);
    assert!(export.warnings.is_empty(), "{:?}", export.warnings);
}

#[test]
fn a_blended_text_keeps_one_style_attribute() {
    let mut t = text("t", "Overlay", 0.0, 0.0);
    t.blend_mode = ImageBlendMode::Multiply;
    let body = layered(&page(vec![t]), "t", &LayerNames::new()).svg;
    assert!(
        body.contains(r#"style="white-space: pre; mix-blend-mode:multiply""#),
        "{body}"
    );
    let text_tag = &body[position(&body, "<text ")..];
    let text_tag = &text_tag[..text_tag.find('>').expect("tag end")];
    assert_eq!(text_tag.matches("style=").count(), 1, "{body}");
}

#[test]
fn a_filter_on_a_fitted_path_reads_the_node_box_space() {
    let mut path = SceneNode::leaf("p", NodeKind::Path);
    path.bounds = Rect::xywh(500.0, 300.0, 40.0, 20.0);
    path.svg_path = Some("M0 0 L4 0 L4 2 Z".into());
    path.fill = Some(Color::BLACK);
    path.effects = vec![Effect::Blur(BlurEffect { radius: 4.0 })];
    let body = layered(&page(vec![path]), "p", &names(&[("p", "Arrow")])).svg;
    // The layer (with the filter) wraps a path that carries the fit.
    assert!(
        body.contains(r#"<g id="Arrow" data-name="Arrow" filter="url(#filter-n-70)"><path d="M0 0 L4 0 L4 2 Z" transform="matrix("#),
        "{body}"
    );
}

#[test]
fn the_view_box_keeps_a_drop_shadow() {
    let mut card = card();
    card.children.clear();
    let body = layered(&page(vec![card]), "1", &card_names()).svg;
    // 12 px blur (σ 6, reach 18) and a 4 px drop below.
    assert!(body.contains(r#"viewBox="-18 -14 356 236""#), "{body}");
}
