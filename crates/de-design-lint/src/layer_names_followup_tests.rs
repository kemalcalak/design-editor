//! #439: navigation only for a header band, no generic section prefixes,
//! badges apart from buttons, and textless siblings told apart by colour
//! or place.

use std::collections::HashMap;

use super::roles_tests::{page_roots, scene_templates, strip_names};
use super::*;
use serde_json::{json, Value};

fn node(value: Value) -> PenNode {
    serde_json::from_value(value).expect("fixture must deserialize as PenNode")
}

fn text(id: &str, content: &str, size: f64) -> Value {
    json!({"type": "text", "id": id, "name": "Text", "content": content, "fontSize": size})
}

fn names_in(roots: &[PenNode], locale: Locale) -> HashMap<String, String> {
    propose_layer_names(roots, None, locale)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect()
}

fn names_of(screen: &PenNode) -> HashMap<String, String> {
    names_in(std::slice::from_ref(screen), Locale::EnUs)
}

fn screen(children: Vec<Value>) -> PenNode {
    node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical", "children": children
    }))
}

// ---- Navigation ----

#[test]
fn a_heading_block_on_top_is_no_navigation() {
    let names = names_of(&screen(vec![
        json!({"type": "frame", "id": "intro", "name": "Frame", "width": 1440, "height": 110,
               "layout": "vertical",
               "children": [text("eyebrow", "Pricing · 3 tiers", 14.0),
                            text("h1", "What the extra money gets you", 28.0),
                            text("sub", "Each tier includes the one before it.", 16.0)]}),
        json!({"type": "frame", "id": "body", "name": "Frame", "width": 1440, "height": 600,
               "layout": "vertical", "children": [text("q", "Questions", 24.0)]}),
    ]));
    assert_ne!(names["intro"], "Navigation");
    assert!(
        names.values().all(|name| !name.starts_with("Navigation")),
        "{names:?}"
    );
}

#[test]
fn links_in_a_row_at_the_top_are_the_navigation() {
    let links: Vec<Value> = ["Product", "Pricing", "Docs"]
        .iter()
        .enumerate()
        .map(|(i, label)| text(&format!("l{i}"), label, 14.0))
        .collect();
    let bar = |id: &str| {
        json!({"type": "frame", "id": id, "name": "Frame", "width": 1440, "height": 72,
               "layout": "horizontal",
               "children": [text(&format!("{id}-logo"), "Reefwright", 20.0),
                            {"type": "frame", "id": format!("{id}-links"), "name": "Frame",
                             "layout": "horizontal", "children": links.clone()}]})
    };
    let names = names_of(&screen(vec![bar("nav")]));
    assert_eq!(names["nav"], "Navigation");

    // A full-width wrapper holding the row alone is the same bar.
    let wrapped = names_of(&screen(vec![json!({
        "type": "frame", "id": "wrap", "name": "Frame", "width": 1440, "layout": "vertical",
        "children": [bar("row")]
    })]));
    assert_eq!(wrapped["wrap"], "Navigation");
}

#[test]
fn a_screen_has_one_hero() {
    let names = names_of(&screen(vec![
        json!({"type": "frame", "id": "a", "name": "Frame", "width": 1440, "height": 300,
               "layout": "vertical", "children": [text("a-h", "Ship faster", 48.0)]}),
        json!({"type": "frame", "id": "b", "name": "Frame", "width": 1440, "height": 300,
               "layout": "vertical", "children": [text("b-h", "Plans", 40.0)]}),
    ]));
    assert_eq!(names["a"], "Hero");
    assert_eq!(names["b"], "Plans section");
}

// ---- Generic section prefixes ----

#[test]
fn a_generic_container_never_prefixes_what_it_holds() {
    let column = |id: &str, name: &str| {
        json!({"type": "frame", "id": id, "name": name, "width": 400, "height": 300,
               "layout": "vertical",
               "children": [text(&format!("{id}-t"), "Book notes", 28.0),
                            text(&format!("{id}-s"), "Five boards", 20.0)]})
    };
    let page = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "horizontal",
        "children": [column("c", "Frame"), column("named", "Reading list")]
    }));
    let names = names_of(&page);
    assert_eq!(names["c"], "Column");
    assert_eq!(names["c-t"], "Title");
    assert_eq!(names["c-s"], "Subtitle");
    // A name the author gave still names the section.
    assert_eq!(names["named-t"], "Reading list / Title");
}

// ---- Badges, buttons and avatars ----

fn label_box(id: &str, extra: Value, label: &str, size: f64) -> Value {
    let mut value = json!({
        "type": "frame", "id": id, "name": "Frame", "layout": "horizontal",
        "fill": [{"type": "solid", "color": "#2563EB"}],
        "children": [text(&format!("{id}-label"), label, size)]
    });
    for (key, field) in extra.as_object().expect("object") {
        value[key] = field.clone();
    }
    value
}

#[test]
fn a_compact_pill_is_a_badge_and_a_roomy_box_a_button() {
    let names = names_of(&screen(vec![json!({
        "type": "frame", "id": "row", "name": "Frame", "layout": "horizontal",
        "children": [
            // `Most popular`: 8 / 18 px around a 22 px label.
            label_box("popular", json!({"padding": [8, 18], "cornerRadius": 999}), "Most popular", 22.0),
            // The shadcn kit's badge: 24 px tall at 12 px.
            label_box("kit-badge", json!({"height": 24, "padding": [0, 10, 0, 10], "cornerRadius": 12}), "New", 12.0),
            // The kit's `md` and `sm` buttons: 40 px at 14 px, 32 px at 13 px.
            label_box("md", json!({"height": 40, "padding": [0, 20, 0, 20], "cornerRadius": 8}), "Get started", 14.0),
            label_box("sm", json!({"height": 32, "padding": [0, 12, 0, 12], "cornerRadius": 8}), "Sign up", 13.0),
            // A pill as roomy as a button around its label.
            label_box("cta", json!({"padding": [10, 20], "cornerRadius": 999}), "Get started", 14.0),
            // A step marker: a 52 px square around `01` at 24 px.
            label_box("step", json!({"width": 52, "height": 52, "cornerRadius": 12}), "01", 24.0)
        ]
    })]));
    assert_eq!(names["popular"], "Badge (Most popular)");
    assert_eq!(names["kit-badge"], "Badge (New)");
    assert_eq!(names["md"], "Primary button (Get started)");
    assert_eq!(names["sm"], "Primary button (Sign up)");
    assert_eq!(names["cta"], "Primary button");
    // `01` holds no word to qualify it with; the plain name is free.
    assert_eq!(names["step"], "Badge");
}

#[test]
fn a_tapped_or_action_box_is_a_button_however_compact() {
    let compact = json!({"padding": [4, 8], "cornerRadius": 999});
    let mut tapped = label_box("tapped", compact.clone(), "Draft", 11.0);
    tapped["events"] = json!({"onTap": [{"scroll_to": {"target": "s"}}]});
    let mut plus = label_box("plus", compact.clone(), "Draft", 11.0);
    plus["children"].as_array_mut().expect("children").insert(
        0,
        json!({"type": "icon_font", "id": "plus-icon", "iconFontName": "lucide:plus"}),
    );
    let mut check = label_box("check", compact, "Verified", 11.0);
    check["children"].as_array_mut().expect("children").insert(
        0,
        json!({"type": "icon_font", "id": "check-icon", "iconFontName": "check"}),
    );
    let names = names_of(&screen(vec![json!({
        "type": "frame", "id": "row", "name": "Frame", "layout": "horizontal",
        "children": [tapped, plus, check]
    })]));
    assert!(names["tapped"].starts_with("Primary button"), "{names:?}");
    assert!(names["plus"].starts_with("Primary button"), "{names:?}");
    assert_eq!(names["check"], "Badge");
}

#[test]
fn a_circle_of_initials_is_an_avatar() {
    let names = names_of(&screen(vec![json!({
        "type": "frame", "id": "row", "name": "Frame", "layout": "horizontal",
        "children": [
            label_box("jd", json!({"width": 40, "height": 40, "cornerRadius": 20}), "JD", 14.0),
            // A rounded square of initials is a mark, not an avatar.
            label_box("mark", json!({"width": 36, "height": 36, "cornerRadius": 11}), "OP", 14.0)
        ]
    })]));
    assert_eq!(names["jd"], "Avatar");
    assert!(!names["mark"].starts_with("Avatar"), "{names:?}");
}

#[test]
fn badge_follows_the_document_language() {
    let names = names_in(
        std::slice::from_ref(&screen(vec![label_box(
            "b",
            json!({"padding": [4, 10], "cornerRadius": 999}),
            "Yeni",
            12.0,
        )])),
        Locale::Tr,
    );
    assert_eq!(names["b"], "Rozet");
}

// ---- Siblings without text ----

fn rect(id: &str, color: &str, extra: Value) -> Value {
    let mut value = json!({"type": "rectangle", "id": id, "name": "Rectangle", "width": 40,
                           "height": 40, "fill": [{"type": "solid", "color": color}]});
    for (key, field) in extra.as_object().expect("object") {
        value[key] = field.clone();
    }
    value
}

fn holder(layout: &str, children: Vec<Value>) -> PenNode {
    node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 400, "height": 400,
        "layout": layout, "children": children
    }))
}

#[test]
fn textless_siblings_are_told_apart_by_colour() {
    let names = names_of(&holder(
        "horizontal",
        vec![
            rect("a", "#2563EB", json!({})),
            rect("b", "#EF4444", json!({})),
            rect("c", "$brand-soft", json!({})),
        ],
    ));
    assert_eq!(names["a"], "Shape (Blue)");
    assert_eq!(names["b"], "Shape (Red)");
    assert_eq!(names["c"], "Shape (brand-soft)");
}

#[test]
fn alike_siblings_are_told_apart_by_place() {
    let gray = "#E5E7EB";
    let row = names_of(&holder(
        "horizontal",
        vec![
            rect("l", gray, json!({})),
            rect("m", gray, json!({})),
            rect("r", gray, json!({})),
        ],
    ));
    assert_eq!(
        [&row["l"], &row["m"], &row["r"]],
        ["Shape (Left)", "Shape (Middle)", "Shape (Right)"]
    );
    let column = names_of(&holder(
        "vertical",
        vec![rect("t", gray, json!({})), rect("b", gray, json!({}))],
    ));
    assert_eq!(
        [&column["t"], &column["b"]],
        ["Shape (Top)", "Shape (Bottom)"]
    );
    // A free-form 2 × 2 grid, listed out of reading order.
    let at = |x: f64, y: f64| json!({"x": x, "y": y});
    let grid = names_of(&holder(
        "none",
        vec![
            rect("br", gray, at(60.0, 60.0)),
            rect("tl", gray, at(0.0, 0.0)),
            rect("tr", gray, at(60.0, 2.0)),
            rect("bl", gray, at(0.0, 60.0)),
        ],
    ));
    assert_eq!(grid["tl"], "Shape (Top left)");
    assert_eq!(grid["tr"], "Shape (Top right)");
    assert_eq!(grid["bl"], "Shape (Bottom left)");
    assert_eq!(grid["br"], "Shape (Bottom right)");
    // Words in the document's language.
    let tr = names_in(
        std::slice::from_ref(&holder(
            "horizontal",
            vec![rect("l", gray, json!({})), rect("r", gray, json!({}))],
        )),
        Locale::Tr,
    );
    assert_eq!([&tr["l"], &tr["r"]], ["Şekil (Sol)", "Şekil (Sağ)"]);
}

#[test]
fn siblings_past_three_across_are_left_to_a_hand_written_name() {
    let gray = "#E5E7EB";
    let wrapped = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 400, "height": 400,
        "layout": "horizontal", "wrap": true,
        "children": [rect("a", gray, json!({})), rect("b", gray, json!({}))]
    }));
    let names = names_of(&wrapped);
    // A wrapping row breaks where the layout decides: no place words.
    assert_eq!(names["a"], "Shape");
    assert!(!names.contains_key("b"));
    let five = names_of(&holder(
        "horizontal",
        ["a", "b", "c", "d", "e"]
            .iter()
            .map(|id| rect(id, gray, json!({})))
            .collect(),
    ));
    assert_eq!(five["a"], "Shape");
    assert_eq!(five.len(), 2, "{five:?}");
}

#[test]
fn lint_suggests_the_place_name_rename_applies() {
    let gray = "#E5E7EB";
    let page = holder(
        "horizontal",
        vec![rect("l", gray, json!({})), rect("r", gray, json!({}))],
    );
    let roots = std::slice::from_ref(&page);
    let renamed = names_of(&page);
    let issues = detect_default_layer_names(&NamingPage::new(roots, &NoInstances), Locale::EnUs);
    for id in ["l", "r"] {
        let issue = issues.iter().find(|i| i.node_id == id).expect("finding");
        assert_eq!(issue.suggested_value, json!(renamed[id]));
    }
}

// ---- Sweep over the scene templates, checked against their authors' names ----

/// Each layer's authored name and role, lowercased and split into words.
fn authored_words(value: &Value, out: &mut HashMap<String, Vec<String>>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get("id").and_then(Value::as_str) {
                let said: String = ["name", "role"]
                    .iter()
                    .filter_map(|key| map.get(*key).and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(" ");
                let words = said
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .map(str::to_string)
                    .collect();
                out.insert(id.to_string(), words);
            }
            map.values().for_each(|child| authored_words(child, out));
        }
        Value::Array(items) => items.iter().for_each(|child| authored_words(child, out)),
        _ => {}
    }
}

/// The detail in a name's closing parentheses (`Closed (%)` in
/// `Column (Closed (%))`).
fn qualifier(name: &str) -> Option<&str> {
    let inner = name.strip_suffix(')')?;
    let mut depth = 0;
    for (at, c) in inner.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' if depth == 0 => return Some(&inner[at + 1..]),
            '(' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn says_any(words: &[String], any: &[&str]) -> bool {
    words.iter().any(|word| any.contains(&word.as_str()))
}

#[test]
fn template_names_follow_what_their_authors_called_them() {
    let en = |key| de_i18n::translate(Locale::EnUs, key);
    // Pills as roomy around their labels as a small button (`VOL.12 Tech
    // Meetup`, `$50 off at launch`, `Lunch break & networking`): their
    // geometry cannot tell them from one (#453).
    let roomy_badges = [
        "conference-agenda-card.de n105",
        "event-invitation-card.de n7",
        "product-launch-card.de n94",
    ];
    // A date filter as compact as a chip (`Last 7 Days` beside a calendar
    // icon), wired to nothing (#453).
    let compact_buttons = ["product-landing-light.de n1445"];
    let mut navigation = Vec::new();
    let (mut badges, mut placed) = (0, 0);
    let (mut misnamed_badges, mut misnamed_buttons) = (Vec::new(), Vec::new());
    for (path, mut document) in scene_templates() {
        let file = path
            .file_name()
            .expect("file")
            .to_string_lossy()
            .to_string();
        let mut authored = HashMap::new();
        authored_words(&document, &mut authored);
        strip_names(&mut document);
        for roots in page_roots(&document) {
            let locale = document_locale(&roots);
            let generic: Vec<String> = [
                "renameLayers.role.row",
                "renameLayers.role.column",
                "renameLayers.role.container",
            ]
            .into_iter()
            .map(|key| {
                de_i18n::translate_with(
                    locale,
                    "renameLayers.role.inSection",
                    &[("section", de_i18n::translate(locale, key)), ("name", "")],
                )
            })
            .collect();
            for p in propose_layer_names(&roots, None, locale) {
                let at = format!("{file} {}", p.node_id);
                let words = authored.get(&p.node_id).cloned().unwrap_or_default();
                assert!(
                    generic
                        .iter()
                        .all(|prefix| !p.proposed.starts_with(prefix.as_str())),
                    "{at}: generic section prefix in {:?}",
                    p.proposed
                );
                if locale != Locale::EnUs {
                    continue;
                }
                if p.proposed == en("renameLayers.role.navigation") {
                    assert!(
                        says_any(&words, &["nav", "navbar", "navigation", "header"]),
                        "{at}"
                    );
                    navigation.push(at.clone());
                }
                let button = p
                    .proposed
                    .starts_with(en("renameLayers.role.primaryButton"))
                    || p.proposed
                        .starts_with(en("renameLayers.role.secondaryButton"));
                if button
                    && says_any(&words, &["badge", "tag", "pill", "chip"])
                    && !roomy_badges.contains(&at.as_str())
                {
                    misnamed_badges.push(format!("{at}: {}", p.proposed));
                }
                if p.proposed.starts_with(en("renameLayers.role.badge")) {
                    if says_any(&words, &["button", "btn", "cta"])
                        && !compact_buttons.contains(&at.as_str())
                    {
                        misnamed_buttons.push(format!("{at}: {}", p.proposed));
                    }
                    badges += 1;
                }
                if let Some(detail) = qualifier(&p.proposed) {
                    assert!(
                        !detail
                            .chars()
                            .all(|c| c.is_ascii_digit() || c.is_whitespace()),
                        "{at}: numbered {:?}",
                        p.proposed
                    );
                    let place = ["Top", "Bottom", "Left", "Right", "Middle", "Center"];
                    placed += usize::from(place.iter().any(|word| detail.starts_with(word)));
                }
            }
        }
    }
    assert!(
        misnamed_badges.is_empty(),
        "badges named as buttons: {misnamed_badges:#?}"
    );
    assert!(
        misnamed_buttons.is_empty(),
        "buttons named as badges: {misnamed_buttons:#?}"
    );
    navigation.sort();
    assert_eq!(
        navigation,
        [
            "product-landing-light.de n1218",
            "saas-landing-orange.de n376"
        ]
    );
    assert!(badges >= 200, "only {badges} badges");
    assert!(placed >= 300, "only {placed} siblings told apart by place");
}
