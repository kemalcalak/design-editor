//! Image fills for the pen.dev conversion. pen.dev keeps bitmaps as files
//! next to the document (`images/x.png`); the editor's in-memory model holds
//! every image as a `data:` URL. Each file is read once through the caller's
//! [`super::AssetReader`], stored once in the document's top-level `images`
//! table, and referenced as `op-image:<id>` — the loader resolves those refs
//! to one shared payload, however many fills use the bitmap.

use base64::Engine as _;
use serde_json::{Map, Value};

use super::image_crop::{self, CropLoss, Fit};
use super::nodes::Ctx;

/// `node` is the filled node's `[width, height]` when both are numbers;
/// a crop fitted with cover / contain needs it.
pub(super) fn convert_image_fill(
    mut obj: Map<String, Value>,
    node: Option<[f64; 2]>,
    ctx: &mut Ctx<'_>,
) -> Option<Value> {
    let Some(Value::String(url)) = obj.remove("url") else {
        ctx.report.note("image fill without a file");
        return None;
    };
    let source = resolve(&url, ctx)?;
    obj.insert("url".to_owned(), Value::String(source));
    // pen.dev defaults to `cover`, which the canonical schema calls `fill`.
    let (mode, fit) = match obj.remove("mode").as_ref().and_then(Value::as_str) {
        Some("contain") => ("fit", Fit::Contain),
        Some("stretch") => ("stretch", Fit::Stretch),
        _ => ("fill", Fit::Cover),
    };
    obj.insert("mode".to_owned(), Value::String(mode.to_owned()));
    if let Some(crop) = obj.remove("transform") {
        let image = ctx.image_sizes.get(&url).copied();
        match image_crop::canonical_transform(&crop, fit, node, image) {
            Ok((transform, loss)) => {
                // A transformed fill is drawn as a crop: the fit is in
                // the matrix.
                obj.insert("mode".to_owned(), Value::String("crop".to_owned()));
                obj.insert("transform".to_owned(), transform);
                match loss {
                    Some(CropLoss::Approximated) => {
                        ctx.report.note("image crop (fitted as stretch)")
                    }
                    Some(CropLoss::Letterbox) => ctx
                        .report
                        .note("image crop (contain shows the rest of the image)"),
                    _ => {}
                }
            }
            Err(_) => ctx.report.note("image crop (invalid transform)"),
        }
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
    let reference = file_bytes(url, ctx.assets).map(|(mime, bytes)| {
        if let Some(size) = de_util::encoded_image_dimensions(&bytes) {
            ctx.image_sizes.insert(url.to_owned(), size);
        }
        let id = format!("pen-dev-{}", ctx.image_table.len());
        ctx.image_table
            .insert(id.clone(), Value::String(data_url(mime, &bytes)));
        format!("{IMAGE_REF_PREFIX}{id}")
    });
    if reference.is_none() {
        ctx.report.note_missing_image(url);
    }
    ctx.images.insert(url.to_owned(), reference.clone());
    reference
}

/// The document-relative file a pen.dev `url` names: a cache-busting query
/// (`a.jpg?v=2`) or a fragment names the same file, `./` is implied.
pub(super) fn relative_path(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    path.trim_start_matches("./")
}

/// The image file `url` names as a `data:` URL, or `None` when it is not a
/// readable image inside the document's folder.
pub(super) fn file_data_url(url: &str, assets: super::AssetReader<'_>) -> Option<String> {
    file_bytes(url, assets).map(|(mime, bytes)| data_url(mime, &bytes))
}

/// The image file `url` names and its mime type, or `None` when it is not a
/// readable image inside the document's folder.
fn file_bytes(url: &str, assets: super::AssetReader<'_>) -> Option<(&'static str, Vec<u8>)> {
    let relative = relative_path(url);
    if !is_inside_document_folder(relative) {
        return None;
    }
    let mime = mime_type(relative)?;
    Some((mime, assets(relative)?))
}

pub fn data_url(mime: &str, bytes: &[u8]) -> String {
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
/// asset folder writer and its prune, and MCP's local image paths (#268),
/// so they cannot drift apart.
pub(crate) const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

pub fn mime_type(relative: &str) -> Option<&'static str> {
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
