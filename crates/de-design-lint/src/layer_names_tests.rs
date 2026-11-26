use std::collections::{HashMap, HashSet};

use super::*;
use serde_json::json;

fn node(value: serde_json::Value) -> PenNode {
    serde_json::from_value(value).expect("fixture must deserialize as PenNode")
}

fn text(id: &str, content: &str, size: f64) -> serde_json::Value {
    json!({"type": "text", "id": id, "name": "Text", "content": content, "fontSize": size})
}

fn card(id: &str, heading: &str) -> serde_json::Value {
    json!({
        "type": "frame", "id": id, "name": "Frame 7", "width": 320, "height": 240,
        "cornerRadius": 12, "layout": "vertical",
        "fill": [{"type": "solid", "color": "#FFFFFF"}],
        "children": [
            text(&format!("{id}-h"), heading, 20.0),
            text(&format!("{id}-p"), "Everything a growing team needs to ship and learn fast.", 14.0)
        ]
    })
}

/// A landing screen the way the generator / an importer leaves it: every
/// layer carries a default name.
fn generated_screen() -> PenNode {
    node(json!({
        "type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 2400,
        "layout": "vertical",
        "children": [
            {"type": "frame", "id": "nav", "name": "Frame 2", "width": 1440, "height": 72,
             "layout": "horizontal",
             "children": [text("nav-1", "Product", 14.0), text("nav-2", "Pricing", 14.0),
                          text("nav-3", "Docs", 14.0)]},
            {"type": "frame", "id": "hero", "name": "Frame 3", "width": 1440, "height": 640,
             "layout": "vertical",
             "children": [
                text("hero-title", "Ship design systems faster", 56.0),
                text("hero-body", "A design tool that writes code your team can actually merge today.", 18.0),
                {"type": "frame", "id": "cta", "name": "Rectangle 4", "width": 160, "height": 48,
                 "cornerRadius": 8, "fill": [{"type": "solid", "color": "#2563EB"}],
                 "children": [text("cta-label", "Get started", 16.0)]},
                {"type": "frame", "id": "cta-2", "name": "Frame 5", "width": 160, "height": 48,
                 "cornerRadius": 8, "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#2563EB"}]},
                 "children": [text("cta-2-label", "Book a demo", 16.0)]},
                {"type": "image", "id": "hero-img", "name": "Image 2", "width": 600, "height": 400,
                 "src": "https://example.com/a.png"}
             ]},
            {"type": "frame", "id": "pricing", "name": "Group 9", "width": 1440, "height": 600,
             "layout": "vertical",
             "children": [
                text("pricing-title", "Pricing", 40.0),
                {"type": "frame", "id": "cards", "name": "Frame 6", "width": 1100, "height": 260,
                 "layout": "horizontal",
                 "children": [card("card-1", "Starter"), card("card-2", "Pro plan"), card("card-3", "Enterprise")]}
             ]},
            {"type": "frame", "id": "footer", "name": "Frame 8", "width": 1440, "height": 120,
             "layout": "horizontal",
             "children": [text("footer-1", "© 2026 Example", 12.0), text("footer-2", "Privacy", 12.0)]}
        ]
    }))
}

fn page_of(root: &PenNode) -> NamingPage<'_> {
    NamingPage::new(std::slice::from_ref(root), &NoInstances)
}

fn names(proposals: &[LayerNameProposal]) -> HashMap<&str, &str> {
    proposals
        .iter()
        .map(|p| (p.node_id.as_str(), p.proposed.as_str()))
        .collect()
}

#[test]
fn default_names_cover_counters_copies_and_other_locales() {
    for name in [
        "Frame",
        "Frame 1",
        "frame 12",
        "Rectangle 3",
        "Group 3 copy 2",
        "Vector",
        "Text Input 2",
        "Çerçeve 2",
        "Dikdörtgen",
        "矩形 1",
        "Rahmen 4",
    ] {
        assert!(is_default_layer_name(name), "{name} is a default");
    }
    for name in [
        "Hero",
        "Pricing card",
        "Frame of mind",
        "Primary button",
        "12",
        "",
    ] {
        assert!(!is_default_layer_name(name), "{name} is not a default");
    }
}

#[test]
fn rules_name_a_generated_screen() {
    let screen = generated_screen();
    let proposals = propose_layer_names(std::slice::from_ref(&screen), None, Locale::EnUs);
    let names = names(&proposals);
    assert_eq!(names["nav"], "Navigation");
    assert_eq!(names["hero"], "Hero");
    assert_eq!(names["hero-title"], "Hero / Title");
    assert_eq!(names["hero-img"], "Hero / Picture");
    assert_eq!(names["cta"], "Primary button");
    assert_eq!(names["cta-2"], "Secondary button");
    assert_eq!(names["pricing"], "Pricing section");
    assert_eq!(names["pricing-title"], "Pricing section / Title");
    assert_eq!(names["cards"], "Pricing cards");
    assert_eq!(names["card-2"], "Pro plan card");
    assert_eq!(names["footer"], "Footer");
    assert_eq!(names["nav-2"], "Pricing");
    assert_eq!(names["cta-label"], "Get started");
    // The body text is too long to be its own name.
    assert_eq!(names["hero-body"], "Hero / Description");
    // No proposal is a default name itself.
    assert!(proposals
        .iter()
        .all(|p| !is_default_layer_name(&p.proposed)));
}

#[test]
fn meaningful_names_are_kept_and_targets_limit_the_walk() {
    let mut value = serde_json::to_value(generated_screen()).expect("json");
    value["children"][1]["name"] = json!("Landing hero");
    let screen = node(value);
    let targets: HashSet<String> = ["hero".to_string()].into_iter().collect();
    let proposals =
        propose_layer_names(std::slice::from_ref(&screen), Some(&targets), Locale::EnUs);
    let names = names(&proposals);
    assert!(!names.contains_key("hero"), "a meaningful name is kept");
    assert!(!names.contains_key("nav"), "outside the targets");
    // Descendants read the kept name as their section.
    assert_eq!(names["hero-title"], "Landing hero / Title");
}

#[test]
fn a_text_named_after_its_content_is_meaningful() {
    let label = node(json!({"type": "text", "id": "t", "name": "Select", "content": "Select"}));
    assert!(has_meaningful_name(&label));
    let renamed = node(json!({"type": "text", "id": "t", "name": "Text 4", "content": "Select"}));
    assert!(!has_meaningful_name(&renamed));
}

#[test]
fn role_words_follow_the_document_language() {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame 1", "width": 400, "height": 800,
        "children": [
            {"type": "frame", "id": "b", "name": "Frame 2", "width": 120, "height": 44,
             "fill": [{"type": "solid", "color": "#111827"}],
             "children": [{"type": "text", "id": "l", "content": "Giriş yap şimdi", "fontSize": 16}]},
            {"type": "text", "id": "p", "content": "Bütün ekibiniz için tasarım ve kod, tek bir yerde ve ücretsiz olarak sunulur.", "fontSize": 14}
        ]
    }));
    let roots = std::slice::from_ref(&screen);
    assert_eq!(document_locale(roots), Locale::Tr);
    let names = propose_layer_names(roots, None, document_locale(roots));
    assert!(names
        .iter()
        .any(|p| p.node_id == "b" && p.proposed == "Birincil düğme"));
    assert!(names
        .iter()
        .any(|p| p.node_id == "p" && p.proposed == "Açıklama"));
}

#[test]
fn document_language_guesses_scripts_and_markers() {
    assert_eq!(detect_document_locale(["Hello world"]), Locale::EnUs);
    assert_eq!(detect_document_locale(["价格方案"]), Locale::ZhCn);
    assert_eq!(detect_document_locale(["料金プランを選ぶ"]), Locale::Ja);
    assert_eq!(detect_document_locale(["요금제"]), Locale::Ko);
    assert_eq!(detect_document_locale(["Цены и тарифы"]), Locale::Ru);
    assert_eq!(detect_document_locale(["Größe und Maße"]), Locale::De);
    assert_eq!(detect_document_locale(["Año de diseño"]), Locale::Es);
    assert_eq!(detect_document_locale([]), Locale::EnUs);
}

#[test]
fn no_role_word_is_a_default_name_in_any_locale() {
    let roles = [
        rules::Role::Screen(None),
        rules::Role::Hero,
        rules::Role::Navigation,
        rules::Role::Footer,
        rules::Role::Section(None),
        rules::Role::Card(None),
        rules::Role::Cards(rules::CardsName::Plain),
        rules::Role::Cards(rules::CardsName::Pricing),
        rules::Role::Subtitle,
        rules::Role::Price,
        rules::Role::Caption,
        rules::Role::Row,
        rules::Role::Column,
        rules::Role::Container,
        rules::Role::PrimaryButton,
        rules::Role::SecondaryButton,
        rules::Role::Title,
        rules::Role::Description,
        rules::Role::Icon(None),
        rules::Role::Picture,
        rules::Role::Avatar,
        rules::Role::Divider,
        rules::Role::Background,
        rules::Role::Dot,
        rules::Role::Illustration,
        rules::Role::Shape,
        rules::Role::InputField,
        rules::Role::Control,
        rules::Role::ProgressBar,
        rules::Role::TabBar,
    ];
    for locale in Locale::ALL {
        for role in &roles {
            let word = role.render(locale, None);
            assert!(
                !word.starts_with("renameLayers."),
                "{locale:?} misses {word}"
            );
            assert!(
                !is_default_layer_name(&word),
                "{locale:?}: role word {word:?} is itself a default name"
            );
        }
    }
}

#[test]
fn lint_reports_default_names_as_info_with_the_rename() {
    let screen = generated_screen();
    let issues = detect_default_layer_names(&page_of(&screen), Locale::EnUs);
    let cta = issues
        .iter()
        .find(|issue| issue.node_id == "cta")
        .expect("default-named button flagged");
    assert_eq!(cta.category, IssueCategory::DefaultLayerName);
    assert_eq!(cta.severity, IssueSeverity::Info);
    assert_eq!(cta.property, FixProperty::Name);
    assert_eq!(cta.current_value, json!("Rectangle 4"));
    assert_eq!(cta.suggested_value, json!("Primary button"));
    let wire = serde_json::to_value(cta).expect("json");
    assert_eq!(wire["category"], "default-layer-name");
    assert_eq!(wire["property"], "name");
    // Text layers named `Text` are flagged; a nameless layer is not.
    assert!(issues.iter().any(|issue| issue.node_id == "nav-1"));
    let nameless = node(json!({"type": "frame", "id": "x", "width": 10, "height": 10}));
    assert!(detect_default_layer_names(&page_of(&nameless), Locale::EnUs).is_empty());
}

fn button_with_fill(color: &str) -> PenNode {
    node(json!({
        "type": "frame", "id": "s", "name": "Frame 1", "width": 400, "height": 400,
        "children": [
            {"type": "frame", "id": "b", "name": "Frame 2", "width": 120, "height": 44,
             "fill": [{"type": "solid", "color": color}],
             "children": [{"type": "text", "id": "l", "content": "Go", "fontSize": 16}]}
        ]
    }))
}

#[test]
fn only_an_alpha_pair_makes_a_button_fill_transparent() {
    for (color, expected) in [
        ("#FF0000", "Primary button"),
        ("#008000", "Primary button"),
        ("#FFA500", "Primary button"),
        ("#2563EBFF", "Primary button"),
        ("#2563EB00", "Secondary button"),
        ("#2563EB33", "Secondary button"),
    ] {
        let screen = button_with_fill(color);
        let proposals = propose_layer_names(std::slice::from_ref(&screen), None, Locale::EnUs);
        assert_eq!(names(&proposals)["b"], expected, "{color}");
    }
}

#[test]
fn localized_group_nouns_are_defaults_but_menu_verbs_are_not() {
    for name in ["Grup 3", "Gruppe", "グループ 2", "Группа 4"] {
        assert!(is_default_layer_name(name), "{name}");
    }
    for name in ["Grupla", "Gruppieren", "Agrupar"] {
        assert!(!is_default_layer_name(name), "{name}");
    }
}
