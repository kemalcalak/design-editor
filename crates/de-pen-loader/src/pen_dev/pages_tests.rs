//! #283: `file://` browser pages from another machine become relative to
//! the document when the page sits next to it.

use serde_json::json;

use super::super::convert;
use super::file_path_segments;

fn reader(relative: &str) -> Option<Vec<u8>> {
    matches!(relative, "video/player.html" | "C# work/page.html").then(|| b"<html></html>".to_vec())
}

fn browser_url(url: &str) -> (String, super::super::PenDevReport) {
    let mut doc = json!({ "version": "2.21", "children": [
        { "type": "browser", "id": "b", "width": 400, "height": 300, "url": url }
    ]});
    let report = convert(&mut doc, &reader);
    (
        doc["children"][0]["url"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        report,
    )
}

#[test]
fn a_page_next_to_the_document_becomes_relative() {
    let (url, report) = browser_url("file:///C:/Users/ana/Site%20Work/video/player.html");
    assert_eq!(url, "./video/player.html");
    assert!(report.missing_pages.is_empty());
    let (url, _) = browser_url("file:///home/ana/site/video/player.html?t=3#start");
    assert_eq!(
        url, "./video/player.html?t=3#start",
        "the query and fragment stay"
    );
}

#[test]
fn a_page_that_is_not_there_is_reported_and_kept() {
    let (url, report) = browser_url("file:///C:/elsewhere/missing.html");
    assert_eq!(url, "file:///C:/elsewhere/missing.html");
    assert_eq!(
        report.missing_pages,
        vec!["file:///C:/elsewhere/missing.html"]
    );
    assert!(report.has_losses());
    assert!(report.short_summary().contains("1 missing page"));
}

#[test]
fn web_addresses_are_left_alone() {
    let (url, report) = browser_url("https://example.com/video/player.html");
    assert_eq!(url, "https://example.com/video/player.html");
    assert!(report.missing_pages.is_empty());
}

#[test]
fn file_paths_decode_and_drop_dot_segments() {
    assert_eq!(
        file_path_segments("FILE:///C:\\a\\..\\b%20c/x.html#top"),
        Some(vec!["C:".into(), "a".into(), "b c".into(), "x.html".into()])
    );
    assert_eq!(file_path_segments("https://x/y"), None);
}

#[test]
fn a_relocated_path_is_escaped_again() {
    let (url, _) = browser_url("file:///D:/C%23%20work/page.html");
    assert_eq!(url, "./C%23%20work/page.html");
}
