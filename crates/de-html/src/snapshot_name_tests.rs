use serde_json::{json, Value};

use super::element_name;

fn name(node: Value) -> Option<String> {
    element_name(node.as_object().unwrap())
}

#[test]
fn an_accessible_label_wins() {
    let node = json!({"tag": "button", "attrs": {"aria-label": "Close dialog", "class": "close"},
        "children": [{"kind": "text", "text": "×"}]});
    assert_eq!(name(node).as_deref(), Some("Close dialog"));
    let image = json!({"tag": "img", "attrs": {"alt": "Team photo"}});
    assert_eq!(name(image).as_deref(), Some("Team photo"));
}

#[test]
fn landmarks_read_as_words() {
    for (tag, word) in [
        ("aside", "Sidebar"),
        ("nav", "Navigation"),
        ("header", "Header"),
    ] {
        assert_eq!(name(json!({"tag": tag})).as_deref(), Some(word));
    }
    let role = json!({"tag": "div", "attrs": {"role": "navigation", "class": "menu"}});
    assert_eq!(name(role).as_deref(), Some("Navigation"));
}

#[test]
fn buttons_links_and_items_take_their_text() {
    let button = json!({"tag": "button", "attrs": {"class": "inline-flex px-4 primary"},
        "children": [{"kind": "image", "tag": "svg"},
                     {"kind": "element", "tag": "span", "children": [{"kind": "text", "text": "New report"}]}]});
    assert_eq!(name(button).as_deref(), Some("New report"));
    let item = json!({"tag": "li", "children": [{"kind": "text", "text": "  Overview "}]});
    assert_eq!(name(item).as_deref(), Some("Overview"));
    let long = json!({"tag": "a", "children": [{"kind": "text",
        "text": "Jira Ekip Kanalına Mesaj Gönderme Talimatları ve Örnekleri"}]});
    let long = name(long).unwrap();
    assert!(
        long.ends_with('\u{2026}') && long.chars().count() <= 40,
        "{long}"
    );
    // A button with no text falls through to its class.
    let icon = json!({"tag": "button", "attrs": {"class": "icon-button"}});
    assert_eq!(name(icon).as_deref(), Some("Icon button"));
}

#[test]
fn ids_and_classes_name_only_when_meaningful() {
    let id = json!({"tag": "div", "attrs": {"id": "billingPanel", "class": "card"}});
    assert_eq!(name(id).as_deref(), Some("Billing panel"));
    let generated =
        json!({"tag": "div", "attrs": {"id": "radix-:r1:", "class": "flex gap-2 title-row"}});
    assert_eq!(name(generated).as_deref(), Some("Title row"));
    let utilities = json!({"tag": "div", "attrs": {
        "class": "flex items-center md:hidden w-[320px] -mt-2 css-1x2y3z text-muted-foreground"}});
    assert_eq!(name(utilities).as_deref(), Some("div"));
    let module = json!({"tag": "section", "attrs": {"class": "card_a8f3k"}});
    assert_eq!(name(module).as_deref(), Some("section"));
}

#[test]
fn a_family_word_is_a_name_unless_a_value_follows() {
    let named = json!({"tag": "div", "attrs": {"class": "content-center list-header"}});
    assert_eq!(name(named).as_deref(), Some("List header"));
    let utilities = json!({"tag": "div", "attrs": {"class": "row-span-2 col-start-1 select-none"}});
    assert_eq!(name(utilities).as_deref(), Some("div"));
    // A hash-shaped word is dropped, even a real one.
    let step = json!({"tag": "div", "attrs": {"class": "step10"}});
    assert_eq!(name(step).as_deref(), Some("div"));
}

#[test]
fn generated_class_hashes_are_never_names() {
    for class in [
        "sc-bdVaJa kQxYzA",
        "page_main__GQzVa",
        "c-PJLV",
        "css-1x2y3z",
    ] {
        let node = json!({"tag": "div", "attrs": {"class": class}});
        assert_eq!(name(node).as_deref(), Some("div"), "{class}");
    }
    let ids = json!({"tag": "div", "attrs": {"id": "mat-select-2", "class": "grayscale"}});
    assert_eq!(name(ids).as_deref(), Some("div"));
}

#[test]
fn words_of_utility_families_still_name() {
    for (class, expected) in [
        ("top-bar", "Top bar"),
        ("text-editor", "Text editor"),
        ("left-panel", "Left panel"),
        ("data-table", "Data table"),
        ("text-muted-foreground order-summary", "Order summary"),
    ] {
        let node = json!({"tag": "div", "attrs": {"class": class}});
        assert_eq!(name(node).as_deref(), Some(expected), "{class}");
    }
    for class in [
        "top-0 left-1/2 table-cell font-semibold text-sm",
        "container-fluid",
    ] {
        let node = json!({"tag": "div", "attrs": {"class": class}});
        assert_eq!(name(node).as_deref(), Some("div"), "{class}");
    }
}

#[test]
fn camel_case_and_acronyms_split_into_words() {
    for (id, expected) in [
        ("URLInput", "Url input"),
        ("FAQSection", "Faq section"),
        ("sidebarNav", "Sidebar nav"),
    ] {
        let node = json!({"tag": "div", "attrs": {"id": id}});
        assert_eq!(name(node).as_deref(), Some(expected), "{id}");
    }
}

#[test]
fn a_vector_icon_keeps_its_icon_name() {
    let svg = json!({"tag": "svg", "attrs": {"class": "lucide lucide-bell"},
        "icon": {"family": "lucide", "names": ["bell"]}});
    assert_eq!(name(svg).as_deref(), Some("bell"));
}

#[test]
fn the_tag_is_the_last_resort() {
    assert_eq!(name(json!({"tag": "div"})).as_deref(), Some("div"));
    assert_eq!(
        name(json!({"tag": "span", "attrs": {}})).as_deref(),
        Some("span")
    );
}
