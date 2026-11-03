//! The import's font step against a scripted font host: which families it
//! asks for, the `@font-face` fallback, renaming and the report.

use std::sync::Mutex;
use std::time::Duration;

use de_editor_core::EditorState;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::*;
use crate::font_host::{FontIssue, FontLoadResult, PageFaceError};

/// Google has Inter and Geist; anything else is not on Google. Page
/// files register under `registers_as` when one is given.
#[derive(Default)]
struct FakeFonts {
    asked: Mutex<Vec<String>>,
    page_files: Mutex<Vec<(String, usize)>>,
    registers_as: Option<&'static str>,
}

impl FontHost for FakeFonts {
    fn load(&self, families: &[String], _wait: Duration) -> Vec<FontLoadResult> {
        self.asked.lock().unwrap().extend(families.iter().cloned());
        families
            .iter()
            .map(|family| FontLoadResult {
                family: family.clone(),
                status: match family.as_str() {
                    "Inter" => FontLoadStatus::Available,
                    "Geist" => FontLoadStatus::Downloaded,
                    _ => FontLoadStatus::NotOnGoogle,
                },
                faces: None,
                detail: None,
            })
            .collect()
    }

    fn font_issues(&self, _state: &EditorState) -> Vec<FontIssue> {
        Vec::new()
    }

    fn register_page_faces(
        &self,
        family: &str,
        faces: Vec<Vec<u8>>,
    ) -> Result<String, PageFaceError> {
        self.page_files
            .lock()
            .unwrap()
            .push((family.to_string(), faces.len()));
        match self.registers_as {
            Some(name) if !faces.is_empty() => Ok(name.to_string()),
            _ => Err(PageFaceError::Unreadable { files: faces.len() }),
        }
    }
}

fn text(id: &str, family: &str) -> serde_json::Value {
    json!({ "type": "text", "id": id, "content": "Hi", "fontFamily": family })
}

fn nodes() -> Vec<PenNode> {
    serde_json::from_value(json!([{
        "type": "frame", "id": "f", "children": [
            text("a", "Geist, \"Geist Fallback\", ui-sans-serif"),
            text("b", "system-ui, Inter, sans-serif"),
            text("c", "geist"),
            text("d", "sans-serif"),
            { "type": "text", "id": "e", "content": [
                { "text": "x", "fontFamily": "'Brand Sans', serif" }
            ] }
        ]
    }]))
    .unwrap()
}

fn snapshot() -> String {
    json!({
        "version": 1,
        "fontFaces": [
            { "family": "Brand Sans", "src": "https://x.test/brand-cyr.woff2",
              "unicodeRange": "U+0400-045F" },
            { "family": "Brand Sans", "src": "https://x.test/brand.woff2",
              "unicodeRange": "U+0000-00FF, U+0131" },
            { "family": "Brand Sans", "src": "https://x.test/brand.woff2" },
            { "family": "Other", "src": "https://x.test/other.woff2" }
        ]
    })
    .to_string()
}

fn no_fetch(urls: Vec<String>) -> Vec<Option<Vec<u8>>> {
    panic!("nothing to fetch, asked for {urls:?}")
}

#[test]
fn families_are_the_first_non_generic_entry_of_each_stack() {
    assert_eq!(
        primary_families(&mut nodes()),
        ["Geist", "Inter", "Brand Sans"]
    );
}

#[test]
fn google_families_load_and_the_rest_come_from_the_page() {
    let host = FakeFonts {
        registers_as: Some("Brand Sans"),
        ..FakeFonts::default()
    };
    let fetched = Mutex::new(Vec::new());
    let fetch = |urls: Vec<String>| {
        fetched.lock().unwrap().extend(urls.iter().cloned());
        urls.iter().map(|_| Some(b"wOF2".to_vec())).collect()
    };
    let mut nodes = nodes();
    let report = load_import_fonts(&host, &mut nodes, &snapshot(), &fetch);
    assert_eq!(report.loaded, ["Geist", "Inter", "Brand Sans"]);
    assert!(report.missing.is_empty(), "{report:?}");
    // The Cyrillic subset is skipped and the file is fetched once.
    assert_eq!(*fetched.lock().unwrap(), ["https://x.test/brand.woff2"]);
    assert_eq!(
        *host.page_files.lock().unwrap(),
        [("Brand Sans".to_string(), 1)]
    );
    assert_eq!(
        *host.asked.lock().unwrap(),
        ["Geist", "Inter", "Brand Sans"]
    );
}

#[test]
fn a_page_file_with_another_name_renames_the_family_in_the_stacks() {
    let host = FakeFonts {
        registers_as: Some("Brand Sans Pro"),
        ..FakeFonts::default()
    };
    let fetch = |urls: Vec<String>| urls.iter().map(|_| Some(vec![1])).collect();
    let mut nodes = nodes();
    let report = load_import_fonts(&host, &mut nodes, &snapshot(), &fetch);
    assert!(report.loaded.contains(&"Brand Sans Pro".to_string()));
    let PenNode::Frame(frame) = &nodes[0] else {
        panic!("frame")
    };
    let PenNode::Text(styled) = &frame.children.as_ref().unwrap()[4] else {
        panic!("text")
    };
    let TextContent::Styled(segments) = &styled.content else {
        panic!("styled")
    };
    assert_eq!(
        segments[0].font_family.as_deref(),
        Some("Brand Sans Pro, serif")
    );
}

#[test]
fn families_with_nothing_to_load_are_reported_missing_with_why() {
    let host = FakeFonts::default();
    let mut nodes = nodes();
    let report = load_import_fonts(&host, &mut nodes, "{}", &no_fetch);
    assert_eq!(report.loaded, ["Geist", "Inter"]);
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].0, "Brand Sans");
    assert!(report.missing[0].1.contains("@font-face"), "{report:?}");

    let fetch = |urls: Vec<String>| urls.iter().map(|_| None).collect();
    let report = load_import_fonts(&host, &mut nodes, &snapshot(), &fetch);
    assert!(
        report.missing[0].1.contains("could not fetch"),
        "{report:?}"
    );

    let fetch = |urls: Vec<String>| urls.iter().map(|_| Some(vec![0])).collect();
    let report = load_import_fonts(&host, &mut nodes, &snapshot(), &fetch);
    assert!(report.missing[0].1.contains("can read"), "{report:?}");

    let mut output = BTreeMap::new();
    report.write(&mut output);
    assert_eq!(output["fontsLoaded"], "Geist, Inter");
    assert!(output["fontsMissing"].starts_with("Brand Sans ("));
}

#[test]
fn more_families_than_one_call_takes_are_reported() {
    let many: Vec<serde_json::Value> = (0..=MAX_FAMILIES)
        .map(|i| text(&format!("t{i}"), &format!("F{i}")))
        .collect();
    let mut nodes: Vec<PenNode> = serde_json::from_value(json!(many)).unwrap();
    let host = FakeFonts::default();
    let report = load_import_fonts(&host, &mut nodes, "{}", &no_fetch);
    assert_eq!(host.asked.lock().unwrap().len(), MAX_FAMILIES);
    assert_eq!(report.missing.len(), MAX_FAMILIES + 1);
    assert!(report.missing[0].1.contains("more than"));
}

#[test]
fn unicode_ranges_and_css_names() {
    assert!(covers_basic_latin(None));
    assert!(covers_basic_latin(Some("U+0000-00FF")));
    assert!(covers_basic_latin(Some("U+00??")));
    assert!(!covers_basic_latin(Some("U+0400-045F, U+0490")));
    assert_eq!(css_family("Brand Sans"), "Brand Sans");
    assert_eq!(css_family("sans-serif"), "sans-serif");
    assert_eq!(css_family("Font 3"), "\"Font 3\"");
}

#[test]
fn fetched_files_decode_and_pad_to_the_request() {
    let reply = json!(["AAEC", null, "!!"]);
    assert_eq!(
        decode_fetched(&reply, 4),
        [Some(vec![0, 1, 2]), None, None, None]
    );
}
