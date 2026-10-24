//! Image fills for the pen.dev conversion. pen.dev keeps bitmaps as files
//! next to the document (`images/x.png`); the editor's in-memory model holds
//! every image as a `data:` URL. Each file is read once through the caller's
//! [`super::AssetReader`], stored once in the document's top-level `images`
//! table, and referenced as `op-image:<id>` — the loader resolves those refs
//! to one shared payload, however many fills use the bitmap.

use base64::Engine as _;
use serde_json::{Map, Value};

use super::nodes::Ctx;

pub(super) fn convert_image_fill(mut obj: Map<String, Value>, ctx: &mut Ctx<'_>) -> Option<Value> {
    let Some(Value::String(url)) = obj.remove("url") else {
        ctx.report.note("image fill without a file");
        return None;
    };
    let source = resolve(&url, ctx)?;
    obj.insert("url".to_owned(), Value::String(source));
    // pen.dev defaults to `cover`, which the canonical schema calls `fill`.
    let mode = match obj.remove("mode").as_ref().and_then(Value::as_str) {
        Some("contain") => "fit",
        Some("stretch") => "stretch",
        _ => "fill",
    };
    obj.insert("mode".to_owned(), Value::String(mode.to_owned()));
    if obj.remove("transform").is_some() {
        ctx.report.note("image crop");
    }
    Some(Value::Object(obj))
}

/// Prefix of a reference into the document's `images` table
/// (`jian_ops_schema::image_table`).
const IMAGE_REF_PREFIX: &str = "op-image:";

fn resolve(url: &str, ctx: &mut Ctx<'_>) -> Option<String> {
    if url.starts_with("data:") || url.starts_with("http://") || url.starts_with("https://") {
        return Some(url.to_owned());
    }
    if let Some(cached) = ctx.images.get(url) {
        return cached.clone();
    }
    // A cache-busting query (`a.jpg?v=2`) or a fragment names the same file.
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let relative = path.trim_start_matches("./");
    let data_url = if is_inside_document_folder(relative) {
        mime_type(relative).and_then(|mime| Some(data_url(mime, &(ctx.assets)(relative)?)))
    } else {
        None
    };
    let reference = data_url.map(|data_url| {
        let id = format!("pen-dev-{}", ctx.image_table.len());
        ctx.image_table.insert(id.clone(), Value::String(data_url));
        format!("{IMAGE_REF_PREFIX}{id}")
    });
    if reference.is_none() {
        ctx.report.note_missing_image(url);
    }
    ctx.images.insert(url.to_owned(), reference.clone());
    reference
}

pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    let prefix = format!("data:{mime};base64,");
    let encoded_len = base64::encoded_len(bytes.len(), true).unwrap_or(0);
    let mut url = String::with_capacity(prefix.len().saturating_add(encoded_len));
    url.push_str(&prefix);
    base64::engine::general_purpose::STANDARD.encode_string(bytes, &mut url);
    url
}

/// A design file must not pull arbitrary files into the document: only
/// plain relative paths below the document's folder are read.
pub(crate) fn is_inside_document_folder(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.starts_with('/')
        && !relative.contains('\\')
        && !relative.contains(':')
        && relative
            .split('/')
            .all(|part| !part.is_empty() && part != "..")
}

/// The image types read from and written to files, as
/// `(extension, mime type)`. One list for the pen.dev reader, the `.de`
/// asset folder writer and its prune, so they cannot drift apart.
pub(crate) const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

pub(crate) fn mime_type(relative: &str) -> Option<&'static str> {
    let extension = relative.rsplit_once('.')?.1.to_ascii_lowercase();
    let extension = if extension == "jpeg" {
        "jpg"
    } else {
        &extension
    };
    IMAGE_TYPES
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, mime)| *mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_paths_are_read() {
        assert!(is_inside_document_folder("images/a.png"));
        assert!(is_inside_document_folder("a.png"));
        assert!(!is_inside_document_folder("../secret.png"));
        assert!(!is_inside_document_folder("images/../../x.png"));
        assert!(!is_inside_document_folder("/etc/x.png"));
        assert!(!is_inside_document_folder("C:/x.png"));
        assert!(!is_inside_document_folder("images\\x.png"));
        assert!(!is_inside_document_folder(""));
    }

    #[test]
    fn mime_type_follows_the_extension() {
        assert_eq!(mime_type("images/A.PNG"), Some("image/png"));
        assert_eq!(mime_type("a.jpeg"), Some("image/jpeg"));
        assert_eq!(mime_type("flow.glsl"), None);
        assert_eq!(mime_type("noext"), None);
    }
}
