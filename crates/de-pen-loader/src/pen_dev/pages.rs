//! Browser pages saved as `file://` addresses (#283).
//!
//! A pen.dev browser node that shows a local page stores the absolute path
//! of the machine that made it (`file:///C:/work/site/video/player.html`).
//! On import the page is looked up next to the document, trying the
//! address's path from its longest tail inward (`work/site/video/…`,
//! `site/video/…`, `video/player.html`, `player.html`): the first that
//! exists becomes a document-relative `./video/player.html` (its query and
//! fragment kept), which loads wherever the folder goes. A page not found
//! is listed in the report. Each candidate is read whole through the asset
//! reader, which is the only file access the converter has.

use serde_json::{Map, Value};

use de_editor_core::browser_live::escape_path_segment;

use super::images::is_inside_document_folder;
use super::nodes::Ctx;

pub(super) fn relocate_file_page(map: &mut Map<String, Value>, ctx: &mut Ctx<'_>) {
    let Some(url) = map.get("url").and_then(Value::as_str) else {
        return;
    };
    let Some(path) = file_path_segments(url) else {
        return;
    };
    let found = (0..path.len()).map(|start| &path[start..]).find(|tail| {
        let candidate = tail.join("/");
        is_inside_document_folder(&candidate) && (ctx.assets)(&candidate).is_some()
    });
    match found {
        Some(tail) => {
            let relative: Vec<String> = tail.iter().map(|s| escape_path_segment(s)).collect();
            let suffix = url.find(['?', '#']).map_or("", |at| &url[at..]);
            let relocated = format!("./{}{suffix}", relative.join("/"));
            map.insert("url".to_owned(), Value::String(relocated));
        }
        None => {
            let url = url.to_owned();
            ctx.report.note_missing_page(&url);
        }
    }
}

/// The decoded path segments of a `file://` address (a Windows drive
/// letter included as the first segment), without its query or fragment.
/// `None` for another scheme.
fn file_path_segments(url: &str) -> Option<Vec<String>> {
    let lower = url.get(..7)?.to_ascii_lowercase();
    if lower != "file://" {
        return None;
    }
    let rest = &url[7..];
    let path = &rest[..rest.find(['?', '#']).unwrap_or(rest.len())];
    let segments: Vec<String> = path
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .map(percent_decode)
        .collect();
    (!segments.is_empty()).then_some(segments)
}

/// `%XX` escapes decoded (UTF-8); a malformed escape stays as written.
fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1), bytes.get(i + 2)) {
            (b'%', Some(&h), Some(&l)) if hex(h).is_some() && hex(l).is_some() => {
                out.push((hex(h).unwrap() * 16 + hex(l).unwrap()) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| segment.to_owned())
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
