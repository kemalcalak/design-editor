//! #433 review: instances, lint and rename agreeing, reading order,
//! hidden layers, screen headings past a header band, priced card rows.

use std::collections::HashMap;

use super::*;
use serde_json::{json, Value};

fn node(value: Value) -> PenNode {
    serde_json::from_value(value).expect("fixture must deserialize as PenNode")
}

fn text(id: &str, content: &str, size: f64) -> Value {
    json!({"type": "text", "id": id, "name": "Text", "content": content, "fontSize": size})
}

fn names_of(roots: &[PenNode]) -> HashMap<String, String> {
    propose_layer_names(roots, None, Locale::EnUs)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect()
}

fn card(id: &str, children: Vec<Value>) -> Value {
    json!({
        "type": "frame", "id": id, "name": "Frame", "width": 240, "height": 200,
        "cornerRadius": 12, "layout": "vertical",
        "fill": [{"type": "solid", "color": "#FFFFFF"}], "children": children
    })
}

/// Masters by id, expanded the way the editor core does for a plain ref.
struct Masters(HashMap<String, PenNode>);

impl InstanceSource for Masters {
    fn expand(&self, instance: &PenNode) -> Option<PenNode> {
        let PenNode::Ref(reference) = instance else {
            return None;
        };
        self.0.get(&reference.target).cloned()
    }
}

#[test]
fn an_instance_is_named_after_the_texts_it_shows() {
    let master = node(card(
        "plan-master",
        vec![text("m-name", "Starter", 20.0), text("m-price", "$9", 28.0)],
    ));
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical",
        "children": [{"type": "frame", "id": "row", "name": "Frame", "width": 1440,
                      "height": 300, "layout": "horizontal",
                      "children": [
                        {"type": "ref", "id": "i1", "ref": "plan-master"},
                        {"type": "ref", "id": "i2", "name": "Frame 3", "ref": "plan-master"}
                      ]}]
    }));
    let roots = std::slice::from_ref(&screen);
    let masters = Masters([("plan-master".to_string(), master)].into_iter().collect());
    let page = NamingPage::new(roots, &masters);
    let names: HashMap<String, String> = propose_layer_names_on(&page, None, Locale::EnUs)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect();
    assert_eq!(names["row"], "Pricing cards");
    // Two instances of one master: the first plain, the second has no
    // text of its own to tell it apart.
    assert_eq!(names["i1"], "Starter card");
    assert!(!names.contains_key("i2"));
    // The master's content is never proposed through an instance.
    assert!(!names.contains_key("m-name") && !names.contains_key("m-price"));
    // Without a source an instance stays unread and unnamed.
    let plain = names_of(roots);
    assert!(!plain.contains_key("i1"));
}

#[test]
fn lint_suggests_exactly_what_rename_applies() {
    let screen = |id: &str| {
        json!({"type": "frame", "id": id, "name": "Frame", "width": 400, "height": 800,
        "children": [
          {"type": "frame", "id": format!("{id}-b"), "name": "Frame 2", "width": 120,
           "height": 44, "fill": [{"type": "solid", "color": "#111827"}],
           "children": [text(&format!("{id}-l"), "Go", 16.0)]}
        ]})
    };
    let roots = vec![node(screen("one")), node(screen("two"))];
    let page = NamingPage::new(&roots, &NoInstances);
    let locale = document_locale(&roots);
    let renamed: HashMap<String, String> = propose_layer_names_on(&page, None, locale)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect();
    let issues = detect_default_layer_names(&page, locale);
    assert!(!issues.is_empty());
    for issue in &issues {
        let expected = renamed
            .get(&issue.node_id)
            .map_or(Value::Null, |name| json!(name));
        assert_eq!(issue.suggested_value, expected, "{}", issue.node_id);
    }
    // The two screens are siblings on the page: never named alike (a lint
    // per screen suggested `Screen` for both). Nothing outside their
    // buttons tells them apart, so the second gets no suggestion.
    assert_eq!(renamed["one"], "Screen");
    assert!(!renamed.contains_key("two"));
    let two = issues.iter().find(|i| i.node_id == "two").expect("flagged");
    assert_eq!(two.suggested_value, Value::Null);
}

#[test]
fn free_form_reading_keeps_rows_and_crosses_groups() {
    // A title inside a group at the top, a price as a sibling below it,
    // and a badge 3 px lower on the title's row but further left.
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 800, "height": 600,
        "children": [
            {"type": "frame", "id": "c", "name": "Frame", "x": 0, "y": 0, "width": 300,
             "height": 240, "cornerRadius": 12, "fill": [{"type": "solid", "color": "#F9FAFB"}],
             "children": [
                {"type": "text", "id": "price", "content": "$29", "fontSize": 32, "x": 16, "y": 90},
                {"type": "group", "id": "g", "name": "Group", "x": 120, "y": 10,
                 "children": [{"type": "text", "id": "plan", "content": "Growth", "fontSize": 24,
                               "x": 0, "y": 0}]},
                {"type": "text", "id": "badge", "content": "Popular pick", "fontSize": 20,
                 "x": 16, "y": 13}
             ]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen));
    // `badge` reads first (same row, further left), so it is the heading.
    assert_eq!(names["c"], "Popular pick card");
    assert_eq!(names["badge"], "Popular pick card / Title");
    assert_eq!(names["plan"], "Popular pick card / Subtitle");
    assert_eq!(names["price"], "Popular pick card / Price");
}

#[test]
fn a_rotated_layer_reads_by_the_box_it_covers() {
    // Turned a quarter about its centre (260, 110), the 200 × 20 label
    // covers y 10…210: its top is above the heading at 40.
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 800, "height": 600,
        "children": [
            {"type": "frame", "id": "c", "name": "Frame", "x": 0, "y": 0, "width": 300,
             "height": 300, "cornerRadius": 12, "fill": [{"type": "solid", "color": "#F9FAFB"}],
             "children": [
                {"type": "text", "id": "plan", "content": "Growth", "fontSize": 24, "x": 40, "y": 40},
                {"type": "text", "id": "side", "content": "Limited offer", "fontSize": 20,
                 "x": 160, "y": 100, "width": 200, "height": 20, "rotation": 90}
             ]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen));
    assert_eq!(names["side"], "Limited offer card / Title");
    assert_eq!(names["plan"], "Limited offer card / Subtitle");
}

#[test]
fn hidden_layers_are_never_headings_or_roles() {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "horizontal",
        "children": [card("c", vec![
            json!({"type": "text", "id": "ghost", "content": "Old plan", "fontSize": 28,
                   "visible": false}),
            text("plan", "Starter", 20.0),
            text("price", "$9", 28.0)
        ])]
    }));
    let names = names_of(std::slice::from_ref(&screen));
    assert_eq!(names["c"], "Starter card");
    assert_eq!(names["plan"], "Starter card / Title");
    assert_eq!(names["ghost"], "Old plan");
}

#[test]
fn a_screen_is_named_past_its_header_band() {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 2000,
        "layout": "vertical",
        "children": [
            {"type": "frame", "id": "nav", "name": "Frame", "width": 1440, "height": 72,
             "layout": "horizontal",
             "children": [
                {"type": "text", "id": "logo", "name": "Text", "content": "Acme",
                 "fontSize": 20, "fontWeight": 700},
                text("l1", "Docs", 14.0), text("l2", "Blog", 14.0)
             ]},
            {"type": "frame", "id": "main", "name": "Frame", "width": 1440, "height": 600,
             "layout": "vertical", "children": [text("h1", "Pricing", 40.0)]}
        ]
    }));
    let names = names_of(std::slice::from_ref(&screen));
    assert_eq!(names["s"], "Pricing screen");
}

#[test]
fn a_mostly_priced_card_row_is_pricing_cards_even_under_a_heading() {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical",
        "children": [{"type": "frame", "id": "sec", "name": "Frame", "width": 1440,
                      "height": 600, "layout": "vertical",
                      "children": [
            text("heading", "Plans", 40.0),
            {"type": "frame", "id": "row", "name": "Frame", "width": 1200, "height": 300,
             "layout": "horizontal",
             "children": [
                card("c1", vec![text("c1-n", "Starter", 20.0), text("c1-p", "99 TL/ay", 28.0)]),
                card("c2", vec![text("c2-n", "Pro", 20.0), text("c2-p", "₺199/ay", 28.0)]),
                card("c3", vec![text("c3-n", "Enterprise", 20.0), text("c3-p", "Talk to us", 16.0)])
             ]}
        ]}]
    }));
    let names = names_of(std::slice::from_ref(&screen));
    assert_eq!(names["row"], "Pricing cards");
    assert_eq!(names["c1-p"], "Starter card / Price");

    // Under half priced: the heading before the row names it.
    let mut value = serde_json::to_value(&screen).expect("json");
    value["children"][0]["children"][1]["children"][1]["children"][1]["content"] = json!("Teams");
    let names = names_of(std::slice::from_ref(&node(value)));
    assert_eq!(names["row"], "Plans cards");
}

#[test]
fn german_rows_of_cards_read_as_compounds() {
    let row = rules::Role::Cards(rules::CardsName::Heading("Tarif".to_string()));
    assert_eq!(row.render(Locale::De, None), "Tarif-Karten");
}
