//! pen.dev `.pen` (schema 2.9+) → canonical `.de` document conversion.
//!
//! pen.dev writes plain JSON close to the canonical schema, but with its own
//! defaults and spellings: frames lay out horizontally unless told otherwise,
//! strokes keep their width / caps beside the paint, effects use
//! `offset { x, y }`, image fills point at files next to the document, and a
//! few node kinds (notes, scripts, live browsers) have no canonical
//! equivalent. [`convert`] rewrites the parsed JSON tree in place before the
//! canonical loader's own legacy repairs run, and returns a [`PenDevReport`]
//! of everything it dropped or approximated so the caller can tell the user.
//!
//! Older pen.dev files (the `2.8` documents the loader has always opened
//! best-effort) keep their historical path: only versions from
//! [`FIRST_PEN_DEV_MINOR`] on take this conversion.

use std::collections::BTreeMap;

use serde_json::Value;

mod glsl;
mod glsl_bindings;
mod glsl_error;
mod glsl_lex;
pub(crate) mod images;
mod nodes;
mod paint;
mod shader;
#[cfg(test)]
mod tests;
mod variables;
#[cfg(test)]
mod variables_tests;

/// Reads a sibling asset (an image fill's relative `url`) for the document
/// being converted. Returns `None` when the file is missing or unreadable;
/// the path is already checked to stay inside the document's folder.
pub type AssetReader<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

/// Reader for callers that have no document folder (a pasted source, a
/// browser upload): every image fill is reported missing.
pub fn no_assets(_relative: &str) -> Option<Vec<u8>> {
    None
}

/// Larger image files are reported missing rather than inlined.
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Reader for the image files a pen.dev document keeps next to itself
/// (`images/x.png`). Paths resolving outside the document's folder (via a
/// symlink) are refused; the converter already rejects `..` and absolute
/// paths before asking.
pub fn folder_assets(document: &std::path::Path) -> impl Fn(&str) -> Option<Vec<u8>> {
    let folder = document
        .parent()
        .and_then(|folder| folder.canonicalize().ok());
    move |relative: &str| {
        let folder = folder.as_ref()?;
        let file = folder.join(relative).canonicalize().ok()?;
        if !file.starts_with(folder) || std::fs::metadata(&file).ok()?.len() > MAX_IMAGE_BYTES {
            return None;
        }
        std::fs::read(file).ok()
    }
}

/// First pen.dev `2.x` minor converted with pen.dev semantics. Earlier 2.x
/// files are the legacy TS-era documents the loader has long opened as v1.
const FIRST_PEN_DEV_MINOR: u32 = 9;

/// Only look this far into the source for the top-level `version`; pen.dev
/// and canonical writers both emit it as the first key.
const VERSION_SNIFF_BYTES: usize = 512;

/// What a pen.dev conversion could not carry over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PenDevReport {
    /// The source document's `version`, e.g. `"2.21"`.
    pub source_version: String,
    /// Dropped or approximated feature → number of occurrences.
    pub unsupported: BTreeMap<&'static str, usize>,
    /// Relative image URLs that could not be read, in first-seen order.
    pub missing_images: Vec<String>,
}

impl PenDevReport {
    /// Whether anything was dropped, approximated or missing.
    pub fn has_losses(&self) -> bool {
        !self.unsupported.is_empty() || !self.missing_images.is_empty()
    }

    /// One-line English list for logs, MCP results and the open dialog's
    /// detail line, e.g. `shader fill ×1, note ×3; missing images: a.png`.
    pub fn summary(&self) -> String {
        let images = if self.missing_images.is_empty() {
            String::new()
        } else {
            format!("missing images: {}", self.missing_images.join(", "))
        };
        self.join_with_features(images)
    }

    /// [`Self::summary`] with the missing images counted instead of named,
    /// for one-line surfaces such as a toast (`shader fill ×1; 14 missing
    /// images`).
    pub fn short_summary(&self) -> String {
        let images = match self.missing_images.len() {
            0 => String::new(),
            1 => "1 missing image".to_owned(),
            count => format!("{count} missing images"),
        };
        self.join_with_features(images)
    }

    fn join_with_features(&self, images: String) -> String {
        let features = self
            .unsupported
            .iter()
            .map(|(feature, count)| format!("{feature} ×{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        match (features.is_empty(), images.is_empty()) {
            (false, false) => format!("{features}; {images}"),
            (false, true) => features,
            (true, false) => images,
            (true, true) => String::new(),
        }
    }

    fn note(&mut self, feature: &'static str) {
        *self.unsupported.entry(feature).or_default() += 1;
    }

    fn note_missing_image(&mut self, url: &str) {
        if !self.missing_images.iter().any(|seen| seen == url) {
            self.missing_images.push(url.to_owned());
        }
    }
}

/// Decide from the raw source whether it is a pen.dev document this module
/// converts: `Some(version)` for a top-level `version` of 2.9 or later with
/// no `formatVersion` (the canonical v1 marker) in the same head. This one
/// rule drives the loader, the save guard and the CLI, so they cannot
/// disagree about a file.
pub fn sniff_pen_dev_version(src: &str) -> Option<String> {
    let head = &src[..floor_char_boundary(src, VERSION_SNIFF_BYTES)];
    if head.contains("\"formatVersion\"") {
        return None;
    }
    let after_key = &head[head.find("\"version\"")? + "\"version\"".len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let quoted = after_colon.strip_prefix('"')?;
    let version = &quoted[..quoted.find('"')?];
    is_pen_dev_version(version).then(|| version.to_owned())
}

/// [`sniff_pen_dev_version`] on the head of a file; `None` when the file is
/// missing or unreadable.
pub fn sniff_pen_dev_file(path: &std::path::Path) -> Option<String> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    file.take(VERSION_SNIFF_BYTES as u64)
        .read_to_end(&mut head)
        .ok()?;
    let head = match std::str::from_utf8(&head) {
        Ok(head) => head,
        // The read may stop inside a multi-byte character.
        Err(error) => std::str::from_utf8(&head[..error.valid_up_to()]).ok()?,
    };
    sniff_pen_dev_version(head)
}

fn is_pen_dev_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse::<u32>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u32>().ok());
    matches!((major, minor), (Some(2), Some(minor)) if minor >= FIRST_PEN_DEV_MINOR)
}

/// Rewrite a parsed pen.dev document into canonical v1 JSON in place.
/// Callers decide with [`sniff_pen_dev_version`] on the source text; a tree
/// whose `version` is not a pen.dev one is left untouched.
pub fn convert(root: &mut Value, assets: AssetReader<'_>) -> PenDevReport {
    let mut report = PenDevReport::default();
    let declared = root.get("version").and_then(Value::as_str);
    if !declared.is_some_and(is_pen_dev_version) {
        return report;
    }
    let Some(doc) = root.as_object_mut() else {
        return report;
    };
    if let Some(Value::String(version)) = doc.remove("version") {
        report.source_version = version;
    }
    doc.insert("version".to_owned(), Value::String("1.0".to_owned()));
    // pen.dev's sync token and cross-file imports mean nothing here.
    doc.remove("fileToken");
    if doc.remove("imports").is_some() {
        report.note("imported .pen libraries");
    }
    let variables = variables::Variables::from_document(doc);
    let mut ctx = nodes::Ctx::new(assets, &mut report, variables);
    if let Some(Value::Array(children)) = doc.get_mut("children") {
        nodes::convert_children(children, false, &mut ctx);
    }
    let image_table = std::mem::take(&mut ctx.image_table);
    if !image_table.is_empty() {
        let table = doc
            .entry("images")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(table) = table {
            table.extend(image_table);
        }
    }
    report
}

fn floor_char_boundary(src: &str, max: usize) -> usize {
    if src.len() <= max {
        return src.len();
    }
    let mut end = max;
    while !src.is_char_boundary(end) {
        end -= 1;
    }
    end
}
