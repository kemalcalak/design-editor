//! HTML deck → document → deck export keeps its entrances (#245).
//!
//! The fixture is one visible slide whose import root becomes the board
//! directly, so the test does not depend on how a multi-slide HTML deck is
//! split into boards (#244).

use std::collections::BTreeMap;
use std::io::Read as _;

use de_editor_core::scene_template_catalog::TemplateScene;
use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::motion::{Easing, NodeAnimation, NodeAnimationFillMode};
use jian_ops_schema::node::PenNode;
use serde_json::json;

const DECK: &str = include_str!("../tests/fixtures/animated-deck.html");

fn imported_deck() -> EditorState {
    let imported = de_html::import_html_document(
        DECK,
        &de_html::HtmlImportOptions {
            viewport_width: 960.0,
            viewport_height: Some(540.0),
            ..Default::default()
        },
        None,
        None,
    );
    assert!(
        imported.diagnostics.is_empty(),
        "the fixture imports cleanly: {:?}",
        imported.warnings
    );
    let mut state = EditorState::from_document(imported.document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    state
}

/// Every animated node by the keyframes signature of its entrance:
/// `(node id, its single animation)`.
fn entrances(state: &EditorState) -> BTreeMap<&'static str, (String, NodeAnimation)> {
    fn walk(node: &PenNode, out: &mut Vec<(String, NodeAnimation)>) {
        if let Some(list) = node.motion_declarations().1 {
            assert_eq!(list.len(), 1, "{}", node.id_str());
            out.push((node.id_str().to_string(), list[0].clone()));
        }
        for child in node.children_ref().into_iter().flatten() {
            walk(child, out);
        }
    }
    let mut found = Vec::new();
    for node in state.active_children() {
        walk(node, &mut found);
    }
    found
        .into_iter()
        .map(|(id, animation)| {
            let from = serde_json::to_value(&animation.keyframes[0].values).expect("values");
            let name = if from == json!({"opacity": 0.0, "translateY": 14.0}) {
                "up"
            } else if from == json!({"opacity": 0.0}) {
                "fade"
            } else if from == json!({"scaleX": 0.0, "scaleY": 0.0}) {
                "grow"
            } else if from == json!({"scaleX": 0.0, "translateX": -200.0}) {
                "drawx"
            } else {
                panic!("unexpected entrance on {id}: {from}");
            };
            (name, (id, animation))
        })
        .collect()
}

#[test]
fn the_import_keeps_each_entrance_with_its_timing() {
    let state = imported_deck();
    let found = entrances(&state);
    let timing: BTreeMap<&str, (u64, u64, Easing)> = found
        .iter()
        .map(|(name, (_, a))| (*name, (a.duration_ms, a.delay_ms, a.easing.clone())))
        .collect();
    assert_eq!(
        timing,
        BTreeMap::from([
            ("up", (600, 0, Easing::CubicBezier(0.2, 0.8, 0.2, 1.0))),
            ("fade", (800, 300, Easing::Ease)),
            ("grow", (500, 450, Easing::EaseOut)),
            ("drawx", (900, 600, Easing::EaseInOut)),
        ])
    );
    assert!(found
        .values()
        .all(|(_, a)| a.fill_mode == NodeAnimationFillMode::Forwards && a.iterations == 1));
}

#[test]
fn the_slideshow_html_replays_the_same_entrances() {
    let state = imported_deck();
    let found = entrances(&state);
    let path = std::env::temp_dir().join(format!(
        "design-editor-roundtrip-{}-{:?}.html",
        std::process::id(),
        std::thread::current().id()
    ));
    crate::export_html::export_deck_html(&state, &path).expect("deck exports");
    let html = std::fs::read_to_string(&path).expect("export written");
    let _ = std::fs::remove_file(&path);

    for (name, expected) in [
        ("up", "600ms cubic-bezier(0.2,0.8,0.2,1) 0ms 1 both"),
        ("fade", "800ms ease 300ms 1 both"),
        ("grow", "500ms ease-out 450ms 1 both"),
        ("drawx", "900ms ease-in-out 600ms 1 both"),
    ] {
        let id = de_codegen::motion_css::css_identifier(&found[name].0);
        assert!(
            html.contains(&format!("data-de-motion-node=\"{id}\"")),
            "{name} has its motion box"
        );
        assert!(
            html.contains(&format!(
                "[data-de-motion-node=\"{id}\"] {{\n  animation: de-{id}-0 {expected};"
            )),
            "{name}: {html}"
        );
    }
    let drawx = de_codegen::motion_css::css_identifier(&found["drawx"].0);
    assert!(
        html.contains(&format!(
            "@keyframes de-{drawx}-0 {{\n  0% {{\n    transform: translate(-200px, 0px) scale(0, 1);"
        )),
        "drawx grows from its left edge: {html}"
    );
}

#[test]
fn the_pptx_plays_the_same_entrances_in_delay_order() {
    let mut state = imported_deck();
    // A transition authored on the imported board travels too.
    state.active_children_mut()[0].base_mut().slide_transition =
        Some(serde_json::from_value(json!({"kind": "fade", "durationMs": 400})).expect("parses"));

    let (bytes, summary) = crate::export_pptx::build_deck_pptx(&state).expect("deck exports");
    assert_eq!(summary.slides, 1);
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    let mut slide = String::new();
    zip.by_name("ppt/slides/slide1.xml")
        .expect("slide part")
        .read_to_string(&mut slide)
        .expect("text");

    assert!(slide.contains("<p:transition spd=\"fast\"><p:fade/></p:transition><p:timing>"));
    // Effects in delay order, with the preset each entrance reads as:
    // Float In (up), Fade, Zoom (grow, drawx).
    let effects: Vec<(String, String)> = slide
        .split(" presetID=\"")
        .skip(1)
        .map(|rest| {
            let preset = rest.split('"').next().expect("preset").to_string();
            let delay = rest
                .split("<p:cond delay=\"")
                .nth(1)
                .and_then(|d| d.split('"').next())
                .expect("delay")
                .to_string();
            (preset, delay)
        })
        .collect();
    let expected = [("42", "0"), ("10", "300"), ("53", "450"), ("53", "600")];
    assert_eq!(
        effects,
        expected.map(|(p, d)| (p.to_string(), d.to_string())),
        "{slide}"
    );
    assert!(
        slide.contains("<p:strVal val=\"#ppt_y+0.0259\"/>"),
        "up rises 14px of 540"
    );
    assert!(
        slide.contains("<p:strVal val=\"#ppt_x-0.2083\"/>"),
        "drawx starts at its edge"
    );
    assert!(
        slide.contains("<p:strVal val=\"#ppt_w*0\"/>"),
        "grow / drawx start at zero"
    );
}
