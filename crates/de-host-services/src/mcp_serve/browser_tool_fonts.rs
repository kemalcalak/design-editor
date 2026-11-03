//! `import-to-canvas` makes the fonts the imported text names available
//! (#301). The page's `font-family` stacks are kept as they are; without
//! their faces text paints in a fallback whose advances move every hugging
//! box. Each family (the first non-generic one of a stack) goes through
//! `load_fonts`' Google Fonts path; one Google does not have is taken from
//! the page's own `@font-face` files, fetched by the page itself and
//! registered for this session. Only a host with a [`FontHost`] (the
//! desktop app) does any of this; the `--serve-web` daemon installs none.

use std::collections::BTreeMap;

use base64::Engine as _;
use de_editor_core::font_catalog::{is_generic_or_system_font_alias, split_font_family_stack};
use de_editor_core::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use serde::Deserialize;

use super::font_tools::{LOAD_WAIT, MAX_FAMILIES};
use crate::font_host::{FontHost, FontLoadStatus};

/// `@font-face` files fetched per family.
const MAX_FILES_PER_FAMILY: usize = 16;

/// What the import did about fonts.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FontReport {
    /// Families text now paints with, as the import names them.
    pub loaded: Vec<String>,
    /// Families still painting with a fallback, with why.
    pub missing: Vec<(String, String)>,
}

impl FontReport {
    /// `fontsLoaded` / `fontsMissing` on the tool's result, when non-empty.
    pub(crate) fn write(&self, output: &mut BTreeMap<String, String>) {
        if !self.loaded.is_empty() {
            output.insert("fontsLoaded".into(), self.loaded.join(", "));
        }
        if !self.missing.is_empty() {
            let missing: Vec<String> = self
                .missing
                .iter()
                .map(|(family, why)| format!("{family} ({why})"))
                .collect();
            output.insert("fontsMissing".into(), missing.join("; "));
        }
    }
}

/// One `@font-face` rule the extractor saw (`snapshot.fontFaces`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageFace {
    pub family: String,
    pub src: String,
    #[serde(default)]
    pub unicode_range: Option<String>,
}

#[derive(Deserialize)]
struct SnapshotFaces {
    #[serde(default, rename = "fontFaces")]
    font_faces: Vec<PageFace>,
}

/// The snapshot's `@font-face` rules (none when it lists none or is not
/// JSON).
pub(crate) fn page_faces(snapshot: &str) -> Vec<PageFace> {
    serde_json::from_str::<SnapshotFaces>(snapshot)
        .map(|faces| faces.font_faces)
        .unwrap_or_default()
}

/// Fetches font files from the page: one entry per url, `None` where the
/// page could not.
pub(crate) type FetchFiles<'a> = &'a dyn Fn(Vec<String>) -> Vec<Option<Vec<u8>>>;

/// Make each family `nodes` use available, renaming a family in the stacks
/// when its page file paints under another name.
pub(crate) fn load_import_fonts(
    host: &dyn FontHost,
    nodes: &mut [PenNode],
    snapshot: &str,
    fetch: FetchFiles<'_>,
) -> FontReport {
    let mut families = primary_families(nodes);
    let mut report = FontReport::default();
    for family in families.drain(MAX_FAMILIES.min(families.len())..) {
        report.missing.push((
            family,
            format!("not loaded: more than {MAX_FAMILIES} families"),
        ));
    }
    if families.is_empty() {
        return report;
    }
    // Families Google cannot supply, with why, for the page's own files.
    let mut from_page: Vec<(String, String)> = Vec::new();
    for result in host.load(&families, LOAD_WAIT) {
        let family = result.family;
        match result.status {
            FontLoadStatus::Available | FontLoadStatus::Cached | FontLoadStatus::Downloaded => {
                report.loaded.push(family);
            }
            FontLoadStatus::Downloading => report.missing.push((
                family,
                "still downloading from Google Fonts; text uses a fallback until it lands".into(),
            )),
            FontLoadStatus::NotOnGoogle => from_page.push((family, "not on Google Fonts".into())),
            FontLoadStatus::Failed => {
                let why = match result.detail {
                    Some(detail) => format!("Google Fonts download failed: {detail}"),
                    None => "Google Fonts download failed".into(),
                };
                from_page.push((family, why));
            }
        }
    }
    if !from_page.is_empty() {
        load_from_page(host, nodes, snapshot, from_page, fetch, &mut report);
    }
    report
}

/// Register `families` from the page's `@font-face` files, fetched in one
/// call. `why` is what Google said, kept in the reason when the page has
/// nothing usable either.
fn load_from_page(
    host: &dyn FontHost,
    nodes: &mut [PenNode],
    snapshot: &str,
    families: Vec<(String, String)>,
    fetch: FetchFiles<'_>,
    report: &mut FontReport,
) {
    let faces = page_faces(snapshot);
    let wanted: Vec<(String, String, Vec<String>)> = families
        .into_iter()
        .map(|(family, why)| {
            let files = face_files(&faces, &family);
            (family, why, files)
        })
        .collect();
    let mut urls: Vec<String> = Vec::new();
    for url in wanted.iter().flat_map(|(_, _, files)| files) {
        if !urls.contains(url) {
            urls.push(url.clone());
        }
    }
    let fetched: Vec<(String, Option<Vec<u8>>)> = if urls.is_empty() {
        Vec::new()
    } else {
        urls.iter().cloned().zip(fetch(urls.clone())).collect()
    };
    for (family, why, files) in wanted {
        if files.is_empty() {
            report.missing.push((
                family,
                format!("{why}; the page has no @font-face file it lets script read"),
            ));
            continue;
        }
        let bytes: Vec<Vec<u8>> = fetched
            .iter()
            .filter(|(url, _)| files.contains(url))
            .filter_map(|(_, bytes)| bytes.clone())
            .collect();
        if bytes.is_empty() {
            report.missing.push((
                family,
                format!("{why}; the page could not fetch its @font-face files"),
            ));
            continue;
        }
        match host.register_page_faces(&family, bytes) {
            Ok(registered) => {
                if !registered.eq_ignore_ascii_case(&family) {
                    rename_family(nodes, &family, &registered);
                }
                report.loaded.push(registered);
            }
            Err(error) => report.missing.push((family, format!("{why}; {error}"))),
        }
    }
}

/// The first non-generic family of every stack imported text uses, in
/// document order, without case-insensitive duplicates.
pub(crate) fn primary_families(nodes: &mut [PenNode]) -> Vec<String> {
    let mut families = Vec::new();
    for_each_stack(nodes, &mut |stack| {
        let Some(family) = stack.as_deref().and_then(|stack| {
            split_font_family_stack(stack)
                .into_iter()
                .find(|family| !is_generic_or_system_font_alias(family))
        }) else {
            return;
        };
        if !families
            .iter()
            .any(|known: &String| known.eq_ignore_ascii_case(&family))
        {
            families.push(family);
        }
    });
    families
}

/// Every `font-family` stack imported text carries: the text node's and its
/// styled segments'. The one walk both reading and renaming use, so they
/// agree on where stacks live.
fn for_each_stack(nodes: &mut [PenNode], visit: &mut dyn FnMut(&mut Option<String>)) {
    for node in nodes {
        if let PenNode::Text(text) = node {
            visit(&mut text.font_family);
            if let TextContent::Styled(segments) = &mut text.content {
                for segment in segments {
                    visit(&mut segment.font_family);
                }
            }
        }
        if let Some(children) = node.children_mut() {
            for_each_stack(children, visit);
        }
    }
}

/// The page files for `family`: one per url, skipping `unicode-range`
/// subsets that leave out basic Latin (a later subset of the same weight
/// would replace the Latin face).
fn face_files(faces: &[PageFace], family: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for face in faces {
        if face.family.eq_ignore_ascii_case(family)
            && covers_basic_latin(face.unicode_range.as_deref())
            && !files.contains(&face.src)
        {
            files.push(face.src.clone());
        }
    }
    files.truncate(MAX_FILES_PER_FAMILY);
    files
}

/// Does a CSS `unicode-range` include `A` (U+0041)? No range means all.
fn covers_basic_latin(range: Option<&str>) -> bool {
    let Some(range) = range.map(str::trim).filter(|range| !range.is_empty()) else {
        return true;
    };
    range.split(',').any(|token| {
        let token = token.trim();
        let Some(hex) = token
            .strip_prefix("U+")
            .or_else(|| token.strip_prefix("u+"))
        else {
            return false;
        };
        let (start, end) = match hex.split_once('-') {
            Some((start, end)) => (start.to_string(), end.to_string()),
            // `U+00??` is a wildcard range.
            None => (hex.replace('?', "0"), hex.replace('?', "F")),
        };
        match (
            u32::from_str_radix(&start, 16),
            u32::from_str_radix(&end, 16),
        ) {
            (Ok(start), Ok(end)) => (start..=end).contains(&0x41),
            _ => false,
        }
    })
}

/// Replace `from` with `to` in every stack imported text uses.
fn rename_family(nodes: &mut [PenNode], from: &str, to: &str) {
    for_each_stack(nodes, &mut |stack| rename_in(stack, from, to));
}

fn rename_in(stack: &mut Option<String>, from: &str, to: &str) {
    let Some(value) = stack.as_deref() else {
        return;
    };
    let families = split_font_family_stack(value);
    if !families
        .iter()
        .any(|family| family.eq_ignore_ascii_case(from))
    {
        return;
    }
    let renamed: Vec<String> = families
        .iter()
        .map(|family| {
            css_family(if family.eq_ignore_ascii_case(from) {
                to
            } else {
                family
            })
        })
        .collect();
    *stack = Some(renamed.join(", "));
}

/// A family as a CSS `font-family` entry: generic keywords and plain
/// identifiers bare, anything else quoted.
fn css_family(family: &str) -> String {
    let plain = family.split(' ').all(|word| {
        word.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    });
    if plain || is_generic_or_system_font_alias(family) {
        family.to_string()
    } else {
        format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// Decode the page's reply to a font fetch: base64 strings or `null`s.
pub(crate) fn decode_fetched(reply: &serde_json::Value, count: usize) -> Vec<Option<Vec<u8>>> {
    let mut files: Vec<Option<Vec<u8>>> = reply
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    entry.as_str().and_then(|data| {
                        base64::engine::general_purpose::STANDARD.decode(data).ok()
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    files.resize(count, None);
    files
}

#[cfg(test)]
#[path = "browser_tool_fonts_tests.rs"]
mod tests;
