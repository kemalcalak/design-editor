//! `capture_preview` (#156): enter preview, click through, capture.

use super::*;

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Two routed screens; the home button pushes the detail screen.
const TWO_SCREENS: &str = r##"{
    "version": "1.1", "formatVersion": "1.1",
    "app": { "name": "x", "version": "1", "id": "x" },
    "pages": [ { "id": "canvas", "name": "Canvas", "children": [
        { "type": "frame", "id": "home", "screen": "/", "x": 0, "y": 0,
          "width": 200, "height": 200, "fill": [{"type":"solid","color":"#ffffff"}],
          "children": [
            { "type": "frame", "id": "go", "x": 20, "y": 60, "width": 120, "height": 40,
              "fill": [{"type":"solid","color":"#2255ff"}],
              "events": { "onTap": [ { "push": "\"/detail\"" } ] } } ] },
        { "type": "frame", "id": "detail", "screen": "/detail", "x": 500, "y": 0,
          "width": 200, "height": 300, "fill": [{"type":"solid","color":"#ff3300"}] }
    ] } ]
}"##;

fn tool() -> CapturePreview {
    let document = jian_ops_schema::load_str(TWO_SCREENS)
        .expect("fixture parses")
        .value;
    capture_preview_snapshot(&EditorState::from_document(document))
}

fn call(tool: &CapturePreview, args: &[(&str, &str)]) -> ToolOutcome {
    let args: BTreeMap<String, String> = args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    tool.call(&args)
}

fn image_and_meta(outcome: ToolOutcome) -> (Vec<u8>, serde_json::Value) {
    match outcome {
        ToolOutcome::OkImageContent {
            image_base64,
            metadata_json,
            ..
        } => (
            base64::engine::general_purpose::STANDARD
                .decode(image_base64)
                .expect("base64"),
            serde_json::from_str(&metadata_json.expect("metadata")).expect("json"),
        ),
        other => panic!("expected an image, got {other:?}"),
    }
}

/// PNG height from the IHDR chunk.
fn png_height(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]])
}

#[test]
fn a_tap_through_captures_the_next_screen() {
    let tool = tool();
    let (entry, meta) = image_and_meta(call(&tool, &[]));
    assert_eq!(&entry[..8], PNG_MAGIC);
    assert_eq!(meta["screen"], "/");
    assert_eq!(meta["nodeId"], "home");

    let (tapped, meta) = image_and_meta(call(&tool, &[("actions", r#"[{"tap":"go"}]"#)]));
    assert_eq!(meta["screen"], "/detail");
    assert_eq!(meta["nodeId"], "detail");
    assert!(
        png_height(&tapped) > png_height(&entry),
        "the 300 px detail screen is captured, not the 200 px home"
    );

    let (_, meta) = image_and_meta(call(
        &tool,
        &[
            ("actions", r#"[{"tap":"go"},{"back":true},{"wait":100}]"#),
            ("timeMs", "250"),
        ],
    ));
    assert_eq!(meta["screen"], "/", "back returns from the pushed screen");
    // `screen` starts on a screen directly.
    let (_, start) = image_and_meta(call(&tool, &[("screen", "/detail")]));
    assert_eq!(start["screen"], "/detail");
    assert!(meta["elapsedMs"].as_u64().expect("elapsed") >= 2 * 400 + 100 + 250);
}

#[test]
fn the_document_is_left_untouched() {
    let document = jian_ops_schema::load_str(TWO_SCREENS)
        .expect("fixture parses")
        .value;
    let state = EditorState::from_document(document);
    let before = serde_json::to_string(&state.doc).expect("serializes");
    let tool = capture_preview_snapshot(&state);
    let _ = image_and_meta(call(&tool, &[("actions", r#"[{"tap":"go"}]"#)]));
    assert_eq!(
        serde_json::to_string(&state.doc).expect("serializes"),
        before
    );
}

#[test]
fn bad_actions_and_unknown_pages_are_invalid_arguments() {
    let tool = tool();
    for (args, needle) in [
        (vec![("actions", "{}")], "JSON array"),
        (vec![("actions", r#"[{"swipe":"go"}]"#)], "unknown action"),
        (
            vec![("actions", r#"[{"tap":"go","hover":"go"}]"#)],
            "one of tap",
        ),
        (
            vec![("actions", r#"[{"tap":"ghost"}]"#)],
            "not on the screen",
        ),
        (vec![("pageId", "nope")], "no page"),
        (vec![("timeMs", "-5")], "timeMs"),
    ] {
        match call(&tool, &args) {
            ToolOutcome::Err(code, message) => {
                assert_eq!(code, ToolErrorCode::InvalidArgument, "{args:?}");
                assert!(message.contains(needle), "{args:?}: {message}");
            }
            other => panic!("{args:?}: expected an error, got {other:?}"),
        }
    }
}
