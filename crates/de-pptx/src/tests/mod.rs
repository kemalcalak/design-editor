//! Import tests over the fixture deck, plus the round trip through the
//! editor's own deck exporters.

mod fixture;
mod present;
mod roundtrip;

use serde_json::Value;

use crate::{import_pptx, PptxError, GRID_GAP};

/// The imported fixture as JSON, so assertions read like the `.de` file.
fn imported() -> (Value, Vec<String>) {
    let import = import_pptx(&fixture::deck(), "Fixture").expect("fixture imports");
    assert_eq!(import.slides, 4);
    let value = serde_json::to_value(&import.document).expect("serialize");
    (value, import.warnings)
}

fn boards(doc: &Value) -> Vec<&Value> {
    doc["children"]
        .as_array()
        .expect("children")
        .iter()
        .filter(|n| n["type"] == "frame")
        .collect()
}

/// Assert a JSON number within a hundredth of a pixel (values are
/// rounded to two decimals; f32 schema fields widen with noise).
#[track_caller]
fn near(value: &Value, expected: f64) {
    let got = value
        .as_f64()
        .unwrap_or_else(|| panic!("not a number: {value}"));
    assert!((got - expected).abs() < 0.011, "{got} != {expected}");
}

/// Depth-first search by node name.
fn find<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    if node["name"] == name {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| find(child, name))
}

fn text_of(node: &Value) -> String {
    match &node["content"] {
        Value::String(s) => s.clone(),
        Value::Array(segments) => segments.iter().filter_map(|s| s["text"].as_str()).collect(),
        _ => String::new(),
    }
}

/// All text in a subtree, in document order.
fn all_text(node: &Value) -> String {
    let mut out = text_of(node);
    for child in node["children"].as_array().into_iter().flatten() {
        out.push_str(&all_text(child));
    }
    out
}

#[test]
fn every_slide_becomes_a_board_at_the_slide_size_in_order() {
    let (doc, _) = imported();
    let boards = boards(&doc);
    assert_eq!(boards.len(), 4);
    let ids: Vec<&str> = boards.iter().filter_map(|b| b["id"].as_str()).collect();
    assert_eq!(ids, ["s1", "s2", "s3", "s4"]);
    for (i, board) in boards.iter().enumerate() {
        near(&board["width"], 1280.0);
        near(&board["height"], 720.0);
        near(&board["x"], i as f64 * (1280.0 + GRID_GAP));
        assert_eq!(board["layout"], "none");
    }
}

#[test]
fn hidden_slides_come_in_hidden_with_their_own_background() {
    let (doc, _) = imported();
    let third = boards(&doc)[2];
    assert_eq!(third["visible"], false);
    assert_eq!(third["fill"][0]["type"], "linear_gradient");
    near(&third["fill"][0]["angle"], 90.0);
    assert_eq!(boards(&doc)[0]["fill"][0]["color"], "#FFFFFF");
}

#[test]
fn placeholders_inherit_box_and_style_from_layout_and_master() {
    let (doc, _) = imported();
    let slide = boards(&doc)[0];
    let title = find(slide, "Title 1").expect("title");
    assert_eq!(title["type"], "text");
    assert_eq!(text_of(title), "Quarterly review");
    // Master box 838200 / 365125 EMU plus the default 0.1" / 0.05" insets.
    near(&title["x"], 88.0 + 9.6);
    near(&title["y"], 38.33 + 4.8);
    // titleStyle: 44 pt bold, heading font, tx2 = dk2.
    assert_eq!(title["fontFamily"], "Georgia");
    near(&title["fontSize"], 58.67);
    assert_eq!(title["fontWeight"], 700);
    assert_eq!(title["content"][0]["fill"], "#1F2937");

    // The body takes the layout's box (moved down), the master's bullets.
    let body = find(slide, "Content 2").expect("body");
    near(&body["y"], 209.97 + 4.8);
    assert_eq!(text_of(body), "• Revenue up\n• Costs flat\n    ▪ Detail");
    assert_eq!(body["fontFamily"], "Inter");
    let detail = body["content"]
        .as_array()
        .expect("segments")
        .iter()
        .find(|s| s["text"] == "Detail")
        .expect("detail run");
    assert_eq!(detail["fontStyle"], "italic");
    near(&detail["fontSize"], 32.0);
}

#[test]
fn master_artwork_sits_behind_the_slide_and_prompts_stay_out() {
    let (doc, _) = imported();
    let slide = boards(&doc)[0];
    let children = slide["children"].as_array().expect("children");
    // Canonical children are front-to-back: the master's artwork is last.
    let last = children.last().expect("master artwork");
    assert_eq!(last["name"], "Accent bar");
    assert_eq!(last["fill"][0]["color"], "#4472C4");
    assert!(!all_text(slide).contains("Master"), "{}", all_text(slide));
}

#[test]
fn speaker_notes_become_a_note_under_the_board() {
    let (doc, _) = imported();
    let note = doc["children"]
        .as_array()
        .expect("children")
        .iter()
        .find(|n| n["type"] == "note")
        .expect("a note");
    assert_eq!(note["id"], "s1-notes");
    assert_eq!(text_of(note), "Open with the numbers.\nThen the plan.");
    near(&note["y"], 720.0 + crate::NOTE_GAP);
}

#[test]
fn shapes_come_in_as_editable_nodes() {
    let (doc, _) = imported();
    let slide = boards(&doc)[1];

    let text_box = find(slide, "TextBox 1").expect("text box");
    assert_eq!(text_box["type"], "text");
    assert_eq!(text_box["textAlign"], "center");
    assert_eq!(text_box["content"][0]["fill"], "#C00000");
    assert_eq!(text_box["content"][0]["fontFamily"], "Arial");
    assert_eq!(text_box["content"][1]["href"], "https://example.com/");
    assert_eq!(text_box["content"][1]["underline"], true);

    // A filled shape with text is a group, front-to-back: its text, then
    // the box behind it.
    let card = find(slide, "Card").expect("card");
    assert_eq!(card["type"], "group");
    let box_node = &card["children"][1];
    assert_eq!(box_node["type"], "rectangle");
    assert_eq!(box_node["fill"][0]["color"], "#ED7D31");
    near(&box_node["cornerRadius"], 14.4);
    let label = &card["children"][0];
    assert_eq!(label["textAlignVertical"], "middle");
    assert_eq!(label["content"][0]["fill"], "#FFFFFF");

    let oval = find(slide, "Oval").expect("oval");
    assert_eq!(oval["type"], "ellipse");
    assert_eq!(oval["fill"][0]["type"], "linear_gradient");
    near(&oval["fill"][0]["angle"], 180.0);
    near(&oval["stroke"]["thickness"], 2.67);
    near(&oval["stroke"]["dashPattern"][0], 10.68);

    // flipV: the segment runs bottom-left to top-right.
    let line = find(slide, "Connector").expect("connector");
    assert_eq!(line["type"], "line");
    near(&line["y"], 128.0 + 100.0);
    near(&line["x2"], 100.0);
    near(&line["y2"], -100.0);

    let logo = find(slide, "Logo").expect("picture");
    assert_eq!(logo["type"], "image");
    assert!(logo["src"]
        .as_str()
        .expect("src")
        .starts_with("data:image/png;base64,"));

    // Child space is 2× the group: the right half starts at 100 px.
    let pair = find(slide, "Pair").expect("group");
    near(&pair["width"], 200.0);
    let right = find(pair, "Right").expect("right");
    near(&right["x"], 100.0);
    near(&right["width"], 100.0);

    // The freeform covers the lower half of its box.
    let freeform = find(slide, "Freeform").expect("path");
    assert_eq!(freeform["type"], "path");
    near(&freeform["y"], 320.0 + 50.0);
    near(&freeform["height"], 50.0);

    let table = find(slide, "Table").expect("table");
    // Children are front-to-back: each cell's text sits before (over) its
    // box, and the first cell ends up last.
    assert_eq!(all_text(table), "1210Q2Q1");
    let cells = table["children"].as_array().expect("cells");
    let first_box = cells.last().expect("first cell");
    assert_eq!(first_box["fill"][0]["color"], "#D9E2F3");
    let first_text = &cells[cells.len() - 2];
    assert_eq!(first_text["type"], "text");
}

#[test]
fn unsupported_content_degrades_with_warnings() {
    let (doc, warnings) = imported();
    let slide = boards(&doc)[1];
    // The chart with a fallback brings its picture.
    assert!(find(slide, "Chart picture").is_some());
    // The bare chart is a labelled placeholder.
    let placeholder = find(slide, "Chart 14").expect("placeholder");
    assert_eq!(all_text(placeholder), "Chart");
    assert!(
        warnings.contains(&"chart (fallback picture)".to_string()),
        "{warnings:?}"
    );
    assert!(
        warnings.contains(&"chart (placeholder)".to_string()),
        "{warnings:?}"
    );
}

#[test]
fn rotation_flips_and_picture_fills_carry_over() {
    let (doc, _) = imported();
    let badge = find(boards(&doc)[1], "Badge").expect("badge");
    assert_eq!(badge["type"], "path");
    near(&badge["rotation"], 90.0);
    assert_eq!(badge["flipX"], true);
    assert_eq!(badge["fill"][0]["type"], "image");
    assert_eq!(badge["fill"][0]["mode"], "stretch");
}

#[test]
fn numbered_lists_count_their_paragraphs() {
    let (doc, _) = imported();
    let steps = find(boards(&doc)[1], "Steps").expect("steps");
    assert_eq!(text_of(steps), "1. Plan\n2. Build\n3. Ship");
}

#[test]
fn smart_art_ole_and_video_degrade_with_warnings() {
    let (doc, warnings) = imported();
    let slide = boards(&doc)[1];
    assert_eq!(
        all_text(find(slide, "Frame 17").expect("smartart box")),
        "SmartArt"
    );
    let preview = find(slide, "Object preview").expect("ole preview");
    assert_eq!(preview["type"], "image");
    near(&preview["x"], 600.0);
    assert_eq!(find(slide, "Clip").expect("video poster")["type"], "image");
    for warning in [
        "smartart (placeholder)",
        "embedded object (preview picture)",
        "video (poster image only)",
    ] {
        assert!(
            warnings.contains(&warning.to_string()),
            "{warning} in {warnings:?}"
        );
    }
}

#[test]
fn a_repeated_picture_is_one_shared_payload() {
    use jian_ops_schema::node::PenNode;
    let import = import_pptx(&fixture::deck(), "Fixture").expect("fixture imports");
    fn images(nodes: &[PenNode], out: &mut Vec<std::sync::Arc<str>>) {
        for node in nodes {
            if let PenNode::Image(image) = node {
                out.push(image.src.as_arc());
            }
            if let Some(kids) = de_editor_core::PenNodeExt::children(node) {
                images(kids, out);
            }
        }
    }
    let mut srcs = Vec::new();
    images(&import.document.children, &mut srcs);
    assert!(srcs.len() >= 3, "{}", srcs.len());
    assert!(srcs
        .windows(2)
        .all(|w| std::sync::Arc::ptr_eq(&w[0], &w[1])));
}

#[test]
fn no_field_is_dropped_on_the_way_into_the_schema() {
    // Every key the converter writes must survive the typed parse: a
    // misspelled field would otherwise vanish without a test noticing.
    let converted = crate::convert(&fixture::deck(), "Fixture").expect("fixture converts");
    let before = converted.value.clone();
    let document = crate::typed(converted.value, converted.images).expect("typed");
    let typed = serde_json::to_value(&document).expect("serialize");
    let mut dropped = Vec::new();
    dropped_keys(&before, &typed, "", &mut dropped);
    assert!(dropped.is_empty(), "{dropped:?}");
}

fn dropped_keys(before: &Value, after: &Value, at: &str, out: &mut Vec<String>) {
    match (before, after) {
        (Value::Object(b), Value::Object(a)) => {
            for (key, value) in b {
                match a.get(key) {
                    Some(kept) => dropped_keys(value, kept, &format!("{at}/{key}"), out),
                    None => out.push(format!("{at}/{key}")),
                }
            }
        }
        (Value::Array(b), Value::Array(a)) => {
            for (i, (value, kept)) in b.iter().zip(a).enumerate() {
                dropped_keys(value, kept, &format!("{at}/{i}"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn boards_wrap_onto_grid_rows_with_room_for_notes() {
    let slide = |notes: Option<&str>| crate::slide::SlideOut {
        board: serde_json::json!({"type": "frame", "id": "s"}),
        notes: notes.map(str::to_string),
    };
    let mut slides: Vec<_> = (0..5).map(|_| slide(None)).collect();
    slides[0] = slide(Some("hi"));
    let out = crate::lay_out(slides, (100.0, 50.0));
    let frames: Vec<&Value> = out.iter().filter(|n| n["type"] == "frame").collect();
    near(&frames[3]["x"], 3.0 * (100.0 + GRID_GAP));
    near(&frames[4]["x"], 0.0);
    near(
        &frames[4]["y"],
        50.0 + GRID_GAP + crate::NOTE_GAP + crate::NOTE_HEIGHT,
    );
}

/// The single animation on the node named `name` of the animated slide.
fn animation<'a>(doc: &'a Value, name: &str) -> &'a Value {
    let node = find(boards(doc)[3], name).unwrap_or_else(|| panic!("{name}"));
    let list = node["animations"]
        .as_array()
        .unwrap_or_else(|| panic!("{name} has animations"));
    assert_eq!(list.len(), 1, "{name}: {list:?}");
    &list[0]
}

#[test]
fn entrances_and_transitions_carry_over() {
    let (doc, warnings) = imported();
    let slide = boards(&doc)[3];
    assert_eq!(slide["slideTransition"]["kind"], "fade");
    assert_eq!(slide["slideTransition"]["durationMs"], 1200);

    let title = animation(&doc, "Motion title");
    assert_eq!(title["trigger"], "mount");
    assert_eq!(title["durationMs"], 500);
    assert!(title.get("delayMs").is_none());
    near(&title["keyframes"][0]["values"]["opacity"], 0.0);
    near(&title["keyframes"][1]["values"]["opacity"], 1.0);

    // After the title: starts when it ends, floats in from 0.25 slide
    // widths (320 px) to the left.
    let floated = animation(&doc, "Float box");
    assert_eq!(floated["delayMs"], 500);
    assert_eq!(floated["durationMs"], 750);
    near(&floated["keyframes"][0]["values"]["translateX"], -320.0);
    near(&floated["keyframes"][1]["values"]["translateX"], 0.0);
    assert_eq!(floated["easing"], "linear");

    // With the box: zooms from half size, eased, played twice.
    let zoomed = animation(&doc, "Zoom circle");
    assert_eq!(zoomed["delayMs"], 500);
    near(&zoomed["keyframes"][0]["values"]["scaleX"], 0.5);
    near(&zoomed["keyframes"][0]["values"]["scaleY"], 0.5);
    assert_eq!(zoomed["easing"], "ease_in_out");
    assert_eq!(zoomed["iterations"], 2);

    assert!(
        warnings.contains(&"emphasis animation (not imported)".to_string()),
        "{warnings:?}"
    );
}

#[test]
fn non_decks_are_refused_with_typed_errors() {
    assert!(matches!(
        import_pptx(b"plain text", "x"),
        Err(PptxError::NotZip(_))
    ));
    let zip = fixture::zip_parts(&[("a.txt".into(), b"hi".to_vec())]);
    assert_eq!(
        import_pptx(&zip, "x").unwrap_err(),
        PptxError::NotPresentation
    );
}
