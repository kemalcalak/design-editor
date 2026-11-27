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

fn is_button(name: &str) -> bool {
    name.contains("button")
}

/// A table of `rows`, each a row of fixed-width boxes around its labels.
fn table(rows: &[&[&str]], widths: &[f64]) -> Value {
    column(
        rows.iter()
            .enumerate()
            .map(|(r, labels)| {
                row(
                    &format!("r{r}"),
                    labels
                        .iter()
                        .zip(widths)
                        .enumerate()
                        .map(|(c, (label, width))| {
                            label_box(&format!("c{r}{c}"), json!(width), label, json!({}))
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

// ---- Table cells ----

#[test]
fn boxes_whose_columns_repeat_across_rows_are_table_cells() {
    let names = names_of(table(
        &[
            &["Spec", "Travel", "Workhorse"],
            &["Weight", "1.24 kg", "1.62 kg"],
        ],
        &[276.0, 330.0, 330.0],
    ));
    assert_eq!(names["c10"], "Cell (Weight)");
    assert!(names["c12"].starts_with("Cell"), "{names:?}");

    // Equal fill-width columns repeat too.
    let fill = |id: &str, label: &str| label_box(id, json!("fill_container"), label, json!({}));
    let names = names_of(column(vec![
        row("r0", vec![fill("a", "Plan"), fill("b", "Seats")]),
        row("r1", vec![fill("c", "Team"), fill("d", "10")]),
    ]));
    assert_eq!(names["c"], "Cell (Team)");
}

#[test]
fn a_lone_row_rounded_boxes_and_a_keypad_are_no_table() {
    // One row of equal boxes is a pair of actions (a dialog's bar), not a
    // table, whatever the labels say.
    let names = names_of(column(vec![row(
        "bar",
        vec![
            label_box("no", json!(160), "Not now", json!({})),
            label_box("ok", json!(160), "Delete", json!({})),
        ],
    )]));
    assert!(is_button(&names["no"]), "{names:?}");
    assert!(is_button(&names["ok"]), "{names:?}");

    // Rounded corners make buttons, even in repeated rows.
    let rounded =
        |id: &str, label: &str| label_box(id, json!(276), label, json!({"cornerRadius": 8}));
    let names = names_of(column(vec![
        row("r1", vec![rounded("x", "Save"), rounded("y", "Cancel")]),
        row("r2", vec![rounded("z", "Back"), rounded("q", "Next")]),
    ]));
    assert!(is_button(&names["x"]), "{names:?}");

    // A grid of single characters is a keypad.
    let names = names_of(table(
        &[&["1", "2", "3"], &["4", "5", "6"], &["7", "8", "9"]],
        &[80.0, 80.0, 80.0],
    ));
    assert!(!names["c00"].starts_with("Cell"), "{names:?}");

    // A box that acts stays a button, and its neighbours stay cells.
    let names = names_of(table(
        &[&["Export", "PDF", "PNG"], &["Share", "Link", "Embed"]],
        &[120.0, 120.0, 120.0],
    ));
    assert!(is_button(&names["c00"]), "{names:?}");
    assert!(names["c01"].starts_with("Cell"), "{names:?}");

    // A calculator: some keys are a single character.
    let names = names_of(table(
        &[&["AC", "+/-", "%", "÷"], &["7", "8", "9", "×"]],
        &[80.0, 80.0, 80.0, 80.0],
    ));
    assert!(!names["c00"].starts_with("Cell"), "{names:?}");
}

#[test]
fn equal_fill_tiles_need_a_value_or_a_third_row_to_be_a_table() {
    let fill = |id: String, label: &str| label_box(&id, json!("fill_container"), label, json!({}));
    let grid = |rows: &[&[&str]]| {
        column(
            rows.iter()
                .enumerate()
                .map(|(r, labels)| {
                    row(
                        &format!("r{r}"),
                        labels
                            .iter()
                            .enumerate()
                            .map(|(c, label)| fill(format!("c{r}{c}"), label))
                            .collect(),
                    )
                })
                .collect(),
        )
    };
    // A share sheet and a size picker: two rows of tiles, no values.
    let share = names_of(grid(&[
        &["Messages", "Notes", "Mail"],
        &["Slack", "Teams", "Drive"],
    ]));
    assert!(!share["c00"].starts_with("Cell"), "{share:?}");
    let sizes = names_of(grid(&[&["XS", "SM", "MD"], &["LG", "XL", "XXL"]]));
    assert!(!sizes["c00"].starts_with("Cell"), "{sizes:?}");
    // A value in a cell, or a third row, makes the grid a table.
    let plans = names_of(grid(&[&["Plan", "Seats"], &["Team", "10"]]));
    assert!(plans["c10"].starts_with("Cell"), "{plans:?}");
    let specs = names_of(grid(&[
        &["Spec", "Travel"],
        &["Display", "Matte"],
        &["Ports", "USB-C"],
    ]));
    assert!(specs["c10"].starts_with("Cell"), "{specs:?}");
}

// ---- Slots ----

#[test]
fn a_placeholder_slot_is_named_for_what_it_holds() {
    let slot = |id: &str, label: &str| {
        label_box(
            id,
            json!(240),
            label,
            json!({"cornerRadius": 16, "height": 64, "justifyContent": "center"}),
        )
    };
    let names = names_of(column(vec![slot("logo", "Drop logo")]));
    assert_eq!(names["logo"], "Logo slot");
    let names = names_of(column(vec![slot("hero", "Your image here")]));
    assert_eq!(names["hero"], "Image slot");
    // Identical slots share the plain name; none is dropped or qualified
    // by the placeholder it shows.
    let names = names_of(row(
        "logos",
        vec![
            slot("l1", "Drop logo"),
            slot("l2", "Drop logo"),
            slot("l3", "Drop logo"),
        ],
    ));
    for id in ["l1", "l2", "l3"] {
        assert_eq!(names[id], "Logo slot", "{names:?}");
    }

    // Other drops, verbs in any language and labels without a kind of
    // content are no slots.
    for label in [
        "Drop shadow",
        "Drop down",
        "Place order",
        "Upload image",
        "Sube tu foto",
        "Ändere dein Bild",
        "点击这里查看图片",
    ] {
        let names = names_of(column(vec![slot("b", label)]));
        assert!(is_button(&names["b"]), "{label}: {names:?}");
    }
}

#[test]
fn an_input_keeps_its_placeholder_text() {
    let field = |label: &str| {
        label_box(
            "field",
            json!(320),
            label,
            json!({"height": 44, "cornerRadius": 6, "fill": [{"type": "solid", "color": "#FFFFFF"}],
                   "stroke": {"thickness": 1, "fill": [{"type": "solid", "color": "#D4D4D8"}]}}),
        )
    };
    for label in ["Your email here", "Your logo here"] {
        let names = names_of(column(vec![field(label)]));
        assert!(!names["field"].contains("lot"), "{label}: {names:?}");
    }
}

/// A box filling its row's width, as tall as a sponsor slot.
fn wide(id: &str, label: &str, extra: Value) -> Value {
    let mut extra = extra;
    extra["height"] = json!(48);
    extra["cornerRadius"] = json!(6);
    label_box(id, json!("fill_container"), label, extra)
}

/// A heading text above a row of three wide boxes.
fn headed_run(heading: &str, labels: [&str; 3]) -> HashMap<String, String> {
    let mut kids = Vec::new();
    if !heading.is_empty() {
        kids.push(json!({"type": "text", "id": "heading", "name": "Text",
                         "content": heading, "fontSize": 14}));
    }
    kids.push(row(
        "r",
        vec![
            wide("a", labels[0], json!({})),
            wide("b", labels[1], json!({})),
            wide("c", labels[2], json!({})),
        ],
    ));
    names_of(column(kids))
}

#[test]
fn a_run_of_identical_boxes_under_its_patrons_is_slots() {
    let names = headed_run("SPECIAL THANKS", ["SOUNDX", "VOLT", "NORTH SHORE"]);
    assert_eq!(names["a"], "Slot (SOUNDX)");
    let names = headed_run("Unsere Partner", ["SOUNDX", "VOLT", "MONO"]);
    assert!(names["a"].starts_with("Slot"), "{names:?}");

    // Tabs and footer links share the shape but nothing introduces patrons.
    for labels in [["DAY", "WEEK", "MONTH"], ["PRIVACY", "TERMS", "CONTACT"]] {
        let names = headed_run("", labels);
        assert!(!names["a"].contains("Slot"), "{labels:?}: {names:?}");
        let names = headed_run("Settings", labels);
        assert!(!names["a"].contains("Slot"), "{labels:?}: {names:?}");
    }
}

#[test]
fn social_sign_in_toolbars_and_segmented_controls_are_no_slots() {
    // A social sign-in row shows a logo in each box.
    let social = |id: &str, label: &str, icon: &str| {
        let mut value = wide(id, label, json!({}));
        value["children"].as_array_mut().expect("children").insert(
            0,
            json!({"type": "icon_font", "id": format!("{id}-i"), "name": "Icon",
                               "iconFontName": icon, "width": 18, "height": 18}),
        );
        value
    };
    let names = names_of(row(
        "r",
        vec![
            social("g", "GOOGLE", "chrome"),
            social("a", "APPLE", "apple"),
            social("h", "GITHUB", "github"),
        ],
    ));
    assert!(is_button(&names["g"]), "{names:?}");

    // A toolbar of equal actions.
    let names = names_of(row(
        "r",
        vec![
            wide("c", "COPY", json!({})),
            wide("s", "SHARE", json!({})),
            wide("e", "EXPORT", json!({})),
        ],
    ));
    assert!(is_button(&names["c"]), "{names:?}");

    // A segmented control marks its selection by a stroke, or by its type.
    let picked =
        json!({"stroke": {"thickness": 2, "fill": [{"type": "solid", "color": "#D8FF00"}]}});
    let names = names_of(row(
        "r",
        vec![
            wide("d", "DAY", json!({})),
            wide("w", "WEEK", json!({})),
            wide("m", "MONTH", picked),
        ],
    ));
    assert!(is_button(&names["d"]), "{names:?}");
    let mut bold = wide("m", "MONTH", json!({}));
    bold["children"][0]["fontWeight"] = json!(700);
    let names = names_of(row(
        "r",
        vec![
            wide("d", "DAY", json!({})),
            wide("w", "WEEK", json!({})),
            bold,
        ],
    ));
    assert!(is_button(&names["d"]), "{names:?}");
}

// ---- Pills ----

fn pill(id: &str, label: &str, extra: Value) -> Value {
    let mut extra = extra;
    extra["cornerRadius"] = json!(999);
    label_box(id, json!("fit_content"), label, extra)
}

fn tracked(mut value: Value) -> Value {
    value["children"][0]["letterSpacing"] = json!(1.2);
    value["children"][0]["fontSize"] = json!(16);
    value
}

#[test]
fn pills_that_label_rather_than_act_are_badges() {
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

    // A price-led pill, in any of the money readers' languages, and a tag
    // in tracked capitals.
    for label in ["$50 off at launch", "Rp 50.000 hemat", "99 TL indirim"] {
        let names = names_of(column(vec![pill("offer", label, json!({}))]));
        assert_eq!(names["offer"], "Badge", "{label}");
    }
    let names = names_of(column(vec![tracked(pill(
        "tag",
        "PRIMARY LOCKUP",
        json!({}),
    ))]));
    assert_eq!(names["tag"], "Badge");
}

#[test]
fn pills_that_act_stay_buttons() {
    // Beside another button, wired to a tap, or a call to action.
    let buy = label_box("buy", json!(200), "Buy now", json!({"cornerRadius": 8}));
    let names = names_of(row("r", vec![pill("offer", "$50 off", json!({})), buy]));
    assert!(is_button(&names["offer"]), "{names:?}");
    let tapped = pill(
        "deal",
        "$50 off at launch",
        json!({"events": {"onTap": [{"scroll_to": {"target": "s"}}]}}),
    );
    let names = names_of(column(vec![tapped]));
    assert!(is_button(&names["deal"]), "{names:?}");
    for label in [
        "GET TICKETS",
        "BUY NOW",
        "RSVP",
        "READ MORE",
        "FREE TRIAL",
        "PRE-ORDER NOW",
        "JETZT KAUFEN",
        "COMPRAR AHORA",
    ] {
        let names = names_of(column(vec![tracked(pill("cta", label, json!({})))]));
        assert!(is_button(&names["cta"]), "{label}: {names:?}");
    }
    // A count is no price.
    for label in ["1-click install", "3 seats left"] {
        let names = names_of(column(vec![pill("count", label, json!({}))]));
        assert!(is_button(&names["count"]), "{label}: {names:?}");
    }
    // A file input: its button leads a row before the file name.
    let choose = label_box("choose", json!(132), "Choose file", json!({"height": 48}));
    let chosen = json!({"type": "text", "id": "chosen", "name": "Text",
                        "content": "No file chosen", "fontSize": 16,
                        "width": "fill_container"});
    let names = names_of(row("r", vec![choose, chosen]));
    assert!(is_button(&names["choose"]), "{names:?}");
}

#[test]
fn cell_and_slot_follow_the_document_language() {
    let names = names_in(
        table(
            &[&["Gewicht", "1,62 kg"], &["Akku", "11 Std."]],
            &[200.0, 200.0],
        ),
        Locale::De,
    );
    assert!(names["c00"].starts_with("Zelle"), "{names:?}");
    let slot = label_box(
        "logo",
        json!(240),
        "Logo hier ablegen",
        json!({"cornerRadius": 16, "height": 64, "justifyContent": "center"}),
    );
    let names = names_in(column(vec![slot]), Locale::De);
    assert_eq!(names["logo"], "Platzhalter Logo");
}
