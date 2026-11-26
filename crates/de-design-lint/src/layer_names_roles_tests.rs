//! #433: headings, distinct text roles, containers of repeated cards and
//! sibling names that never repeat.

use std::collections::{HashMap, HashSet};

use super::*;
use crate::node_util::node_id;
use serde_json::{json, Value};

fn node(value: Value) -> PenNode {
    serde_json::from_value(value).expect("fixture must deserialize as PenNode")
}

fn text(id: &str, content: &str, size: f64) -> Value {
    json!({"type": "text", "id": id, "name": "Text", "content": content, "fontSize": size})
}

fn names_of(roots: &[PenNode], locale: Locale) -> HashMap<String, String> {
    propose_layer_names(roots, None, locale)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect()
}

const PLANS: [(&str, &str); 6] = [
    ("Starter", "$9"),
    ("Basic", "$12"),
    ("Plus", "$19"),
    ("Pro", "$29"),
    ("Team", "$49"),
    ("Lite", "$5"),
];

/// The grid from the issue: six cards, plan name 20 px, price 28 px and a
/// button, directly under the screen.
fn pricing_grid() -> PenNode {
    let cards: Vec<Value> = PLANS
        .iter()
        .enumerate()
        .map(|(i, (plan, price))| {
            json!({
                "type": "frame", "id": format!("card-{i}"), "name": "Frame", "width": 220,
                "height": 320, "cornerRadius": 16, "layout": "vertical",
                "fill": [{"type": "solid", "color": "#FFFFFF"}],
                "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#E5E7EB"}]},
                "children": [
                    text(&format!("card-{i}-name"), plan, 20.0),
                    text(&format!("card-{i}-price"), price, 28.0),
                    {"type": "frame", "id": format!("card-{i}-cta"), "name": "Frame",
                     "width": 180, "height": 44, "cornerRadius": 8,
                     "fill": [{"type": "solid", "color": "#2563EB"}],
                     "children": [text(&format!("card-{i}-cta-label"), "Choose plan", 16.0)]}
                ]
            })
        })
        .collect();
    node(json!({
        "type": "frame", "id": "screen", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical",
        "children": [
            {"type": "frame", "id": "grid", "name": "Frame", "width": 1440, "height": 700,
             "layout": "horizontal", "children": cards}
        ]
    }))
}

#[test]
fn the_pricing_grid_is_named_after_plans_not_prices() {
    let grid = pricing_grid();
    let names = names_of(std::slice::from_ref(&grid), Locale::EnUs);
    assert_eq!(names["card-0"], "Starter card");
    assert_eq!(names["card-0-name"], "Starter card / Title");
    assert_eq!(names["card-0-price"], "Starter card / Price");
    assert_eq!(names["card-0-cta"], "Primary button");
    assert_eq!(names["card-0-cta-label"], "Choose plan");
    assert_eq!(names["card-5"], "Lite card");
    assert_eq!(names["card-5-price"], "Lite card / Price");
    assert_eq!(names["grid"], "Pricing cards");
    assert_eq!(names["screen"], "Screen");
}

#[test]
fn pricing_roles_follow_the_document_language() {
    let grid = pricing_grid();
    let names = names_of(std::slice::from_ref(&grid), Locale::Tr);
    assert_eq!(names["card-0"], "Starter kartı");
    assert_eq!(names["card-0-name"], "Starter kartı / Başlık");
    assert_eq!(names["card-0-price"], "Starter kartı / Fiyat");
    assert_eq!(names["grid"], "Fiyat kartları");
}

#[test]
fn free_form_cards_read_top_to_bottom() {
    // The price comes first in the tree but sits under the plan name.
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 800, "height": 600,
        "children": [
            {"type": "frame", "id": "c", "name": "Frame", "x": 40, "y": 40, "width": 240,
             "height": 200, "cornerRadius": 12, "fill": [{"type": "solid", "color": "#F9FAFB"}],
             "children": [
                {"type": "text", "id": "price", "content": "€29/mo", "fontSize": 32, "x": 16, "y": 64},
                {"type": "text", "id": "plan", "content": "Growth", "fontSize": 20, "x": 16, "y": 16}
             ]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen), Locale::EnUs);
    assert_eq!(names["c"], "Growth card");
    assert_eq!(names["plan"], "Growth card / Title");
    assert_eq!(names["price"], "Growth card / Price");
}

#[test]
fn a_stat_card_is_named_after_its_label_not_its_number() {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 800, "height": 600,
        "layout": "horizontal",
        "children": [
            {"type": "frame", "id": "stat", "name": "Frame", "width": 200, "height": 120,
             "cornerRadius": 12, "layout": "vertical",
             "fill": [{"type": "solid", "color": "#F9FAFB"}],
             "children": [text("value", "12k", 40.0), text("label", "Active users", 14.0)]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen), Locale::EnUs);
    assert_eq!(names["stat"], "Active users card");
    assert_eq!(names["value"], "12k");
    assert_eq!(names["label"], "Active users");
}

#[test]
fn texts_of_one_unit_get_distinct_roles() {
    let long = "A design tool that writes code your team can actually merge today.";
    let fine = "Prices exclude VAT. Cancel any time from the billing page of your account.";
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical",
        "children": [
            {"type": "frame", "id": "hero", "name": "Frame", "width": 1440, "height": 640,
             "layout": "vertical",
             "children": [
                text("h1", "Ship design systems faster", 56.0),
                text("h2", "Design and code in one place", 28.0),
                text("h3", "For every team", 24.0),
                text("p1", long, 18.0),
                text("p2", &format!("{long} Again."), 18.0),
                text("small", fine, 12.0)
             ]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen), Locale::EnUs);
    assert_eq!(names["h1"], "Hero / Title");
    assert_eq!(names["h2"], "Hero / Subtitle");
    assert_eq!(names["h3"], "For every team");
    assert_eq!(names["p1"], "Hero / Description");
    assert_eq!(names["p2"], "A design tool that writes code…");
    assert_eq!(names["small"], "Hero / Caption");
}

#[test]
fn repeated_siblings_are_told_apart_or_left_alone() {
    let button = |id: &str, label: &str| {
        json!({"type": "frame", "id": id, "name": "Frame", "width": 160, "height": 44,
               "cornerRadius": 8, "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#2563EB"}]},
               "children": [text(&format!("{id}-label"), label, 16.0)]})
    };
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 400, "height": 400,
        "children": [
            button("a", "Book a demo"),
            button("b", "Contact sales"),
            button("c", "Book a demo"),
            {"type": "frame", "id": "kept", "name": "Secondary button (Docs)", "width": 10, "height": 10},
            button("d", "Docs")
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen), Locale::EnUs);
    assert_eq!(names["a"], "Secondary button");
    assert_eq!(names["b"], "Secondary button (Contact sales)");
    assert_eq!(names["c"], "Secondary button (Book a demo)");
    // Its qualified name is a kept sibling's, and nothing else tells it apart.
    assert!(!names.contains_key("d"), "{:?}", names.get("d"));
}

// ---- Sweep: no two siblings ever get the same proposed name. ----

/// Drop every layer name, so the rules propose one for every layer.
fn strip_names(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("name");
            for child in map.values_mut() {
                strip_names(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_names),
        _ => {}
    }
}

fn assert_distinct_siblings(label: &str, roots: &[PenNode], proposed: &HashMap<String, String>) {
    fn check(label: &str, nodes: &[PenNode], proposed: &HashMap<String, String>) {
        let mut seen = HashSet::new();
        for node in nodes {
            if let Some(name) = proposed.get(node_id(node)) {
                assert!(
                    seen.insert(name.to_lowercase()),
                    "{label}: two siblings proposed as {name:?}"
                );
            }
            check(label, children(node), proposed);
        }
    }
    check(label, roots, proposed);
}

fn sweep(label: &str, roots: &[PenNode]) -> usize {
    let locale = document_locale(roots);
    let proposals = propose_layer_names(roots, None, locale);
    let proposed: HashMap<String, String> = proposals
        .iter()
        .map(|p| (p.node_id.clone(), p.proposed.clone()))
        .collect();
    assert_distinct_siblings(label, roots, &proposed);
    for p in &proposals {
        assert!(
            !is_default_layer_name(&p.proposed),
            "{label}: {} proposed as the default {:?}",
            p.node_id,
            p.proposed
        );
    }
    proposals.len()
}

fn page_roots(document: &Value) -> Vec<Vec<PenNode>> {
    let decode = |nodes: &Value| -> Vec<PenNode> {
        nodes
            .as_array()
            .map(|nodes| nodes.iter().cloned().map(node).collect())
            .unwrap_or_default()
    };
    let mut pages: Vec<Vec<PenNode>> = document["pages"]
        .as_array()
        .map(|pages| pages.iter().map(|page| decode(&page["children"])).collect())
        .unwrap_or_default();
    pages.push(decode(&document["children"]));
    pages
}

#[test]
fn no_two_siblings_share_a_proposed_name_on_generated_screens() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../de-editor-core/assets/scene_templates");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("scene templates")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "de"))
        .collect();
    files.sort();
    assert!(files.len() >= 50, "found {} scene templates", files.len());
    let mut proposals = 0;
    for path in &files {
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json");
        strip_names(&mut document);
        for roots in page_roots(&document) {
            proposals += sweep(&path.display().to_string(), &roots);
        }
    }
    proposals += sweep("pricing grid", std::slice::from_ref(&pricing_grid()));
    assert!(proposals > 1000, "only {proposals} proposals swept");
}
