//! #453: a label box read with its neighbours: table cells, slots and
//! pills that look like buttons on their own.

use std::collections::HashMap;

use super::*;
use serde_json::{json, Value};

fn node(value: Value) -> PenNode {
    serde_json::from_value(value).expect("fixture must deserialize as PenNode")
}

fn names_in(screen: Value, locale: Locale) -> HashMap<String, String> {
    let screen = node(json!({
        "type": "frame", "id": "s", "name": "Frame", "width": 1440, "height": 900,
        "layout": "vertical", "children": [screen]
    }));
    propose_layer_names(std::slice::from_ref(&screen), None, locale)
        .into_iter()
        .map(|p| (p.node_id, p.proposed))
        .collect()
}

fn names_of(screen: Value) -> HashMap<String, String> {
    names_in(screen, Locale::EnUs)
}

/// A filled box around one label, as wide as `width`.
fn label_box(id: &str, width: Value, label: &str, extra: Value) -> Value {
    let mut value = json!({
        "type": "frame", "id": id, "name": "Frame", "width": width,
        "layout": "horizontal", "padding": [20, 22],
        "fill": [{"type": "solid", "color": "#1F2937"}],
        "children": [{"type": "text", "id": format!("{id}-t"), "name": "Text",
                      "content": label, "fontSize": 20}]
    });
    if let (Some(map), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        map.extend(extra.clone());
    }
    value
}

fn row(id: &str, kids: Vec<Value>) -> Value {
    json!({"type": "frame", "id": id, "name": "Frame", "width": "fill_container",
           "layout": "horizontal", "gap": 4, "children": kids})
}

fn column(kids: Vec<Value>) -> Value {
    json!({"type": "frame", "id": "col", "name": "Frame", "width": 940,
           "layout": "vertical", "gap": 4, "children": kids})
}

#[test]
fn boxes_whose_columns_line_up_across_rows_are_table_cells() {
    let spec_row = |id: &str, cells: [&str; 3]| {
        row(
            id,
            vec![
                label_box(&format!("{id}a"), json!(276), cells[0], json!({})),
                label_box(&format!("{id}b"), json!(330), cells[1], json!({})),
                label_box(&format!("{id}c"), json!(330), cells[2], json!({})),
            ],
        )
    };
    let names = names_of(column(vec![
        spec_row("h", ["Spec", "Travel", "Workhorse"]),
        spec_row("w", ["Weight", "1.24 kg", "1.62 kg"]),
    ]));
    assert_eq!(names["wa"], "Cell (Weight)");
    assert!(names["wc"].starts_with("Cell"), "{names:?}");

    // The same boxes with rounded corners are buttons again.
    let rounded =
        |id: &str, label: &str| label_box(id, json!(276), label, json!({"cornerRadius": 8}));
    let names = names_of(column(vec![
        row("r1", vec![rounded("x", "Save"), rounded("y", "Cancel")]),
        row("r2", vec![rounded("z", "Back"), rounded("q", "Next")]),
    ]));
    assert!(names["x"].contains("button"), "{names:?}");
}

#[test]
fn equal_boxes_filling_a_row_are_cells_but_hugging_ones_are_not() {
    let names = names_of(row(
        "r",
        vec![
            label_box("a", json!("fill_container"), "Weight", json!({})),
            label_box("b", json!("fill_container"), "1.62 kg", json!({})),
        ],
    ));
    assert_eq!(names["a"], "Cell (Weight)");

    let names = names_of(row(
        "r",
        vec![
            label_box("a", json!("fit_content"), "Weight", json!({})),
            label_box("b", json!("fit_content"), "1.62 kg", json!({})),
        ],
    ));
    assert!(!names["a"].starts_with("Cell"), "{names:?}");
}

#[test]
fn a_placeholder_slot_is_named_for_what_it_holds() {
    let slot = |id: &str, label: &str| {
        label_box(
            id,
            json!(240),
            label,
            json!({"cornerRadius": 16, "height": 64}),
        )
    };
    let names = names_of(column(vec![slot("logo", "Drop logo")]));
    assert_eq!(names["logo"], "Logo slot");
    let names = names_of(column(vec![slot("hero", "Your image here")]));
    assert_eq!(names["hero"], "Image slot");
    // A verb that presses keeps the button.
    let names = names_of(column(vec![slot("order", "Place order")]));
    assert!(names["order"].contains("button"), "{names:?}");
}

#[test]
fn a_run_of_identical_boxes_filling_a_row_is_slots() {
    let sponsor = |id: &str, label: &str| {
        label_box(
            id,
            json!("fill_container"),
            label,
            json!({"height": 48, "cornerRadius": 6,
                   "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#2E1B45"}]}}),
        )
    };
    let names = names_of(row(
        "r",
        vec![
            sponsor("a", "SOUNDX"),
            sponsor("b", "VOLT"),
            sponsor("c", "NORTH SHORE"),
        ],
    ));
    assert_eq!(names["a"], "Slot (SOUNDX)");

    // A segmented control: one box is selected, so they differ.
    let mut picked = sponsor("c", "Month");
    picked["fill"] = json!([{"type": "solid", "color": "#D8FF00"}]);
    let names = names_of(row(
        "r",
        vec![sponsor("a", "Day"), sponsor("b", "Week"), picked],
    ));
    assert!(!names["a"].contains("Slot"), "{names:?}");
}

#[test]
fn pills_that_label_rather_than_act_are_badges() {
    let pill = |id: &str, label: &str, extra: Value| {
        let mut extra = extra;
        extra["cornerRadius"] = json!(999);
        label_box(id, json!("fit_content"), label, extra)
    };
    // A chip that leads its row of text (`Main` before the line-up).
    let chip = label_box(
        "chip",
        json!(132),
        "Main",
        json!({"height": 42, "cornerRadius": 4}),
    );
    let artists = json!({"type": "text", "id": "artists", "name": "Text",
                         "content": "Neon Tide, Glass Harbor", "fontSize": 20,
                         "width": "fill_container"});
    let names = names_of(row("r", vec![chip, artists]));
    assert_eq!(names["chip"], "Badge");

    // An amount-led pill and a tracked capitals one.
    let names = names_of(column(vec![pill("offer", "$50 off at launch", json!({}))]));
    assert_eq!(names["offer"], "Badge");
    let mut caps = pill("tag", "PRIMARY LOCKUP", json!({}));
    caps["children"][0]["letterSpacing"] = json!(1.2);
    caps["children"][0]["fontSize"] = json!(16);
    let names = names_of(column(vec![caps.clone()]));
    assert_eq!(names["tag"], "Badge");

    // Beside another button, or wired to a tap, it stays a button.
    let buy = label_box("buy", json!(200), "Buy now", json!({"cornerRadius": 8}));
    let names = names_of(row("r", vec![pill("offer", "$50 off", json!({})), buy]));
    assert!(names["offer"].contains("button"), "{names:?}");
    let tapped = pill(
        "deal",
        "$50 off at launch",
        json!({"events": {"onTap": [{"scroll_to": {"target": "s"}}]}}),
    );
    let names = names_of(column(vec![tapped]));
    assert!(names["deal"].contains("button"), "{names:?}");
}

#[test]
fn cell_and_slot_follow_the_document_language() {
    let names = names_in(
        row(
            "r",
            vec![
                label_box("a", json!(200), "Gewicht", json!({})),
                label_box("b", json!(200), "1,62 kg", json!({})),
            ],
        ),
        Locale::De,
    );
    assert!(names["a"].starts_with("Zelle"), "{names:?}");
}
