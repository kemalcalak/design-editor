//! Image fills for the pen.dev conversion. pen.dev keeps bitmaps as files
//! next to the document (`images/x.png`); the editor's in-memory model holds
//! every image as a `data:` URL, so each relative URL is read through the
//! caller's [`super::AssetReader`] and inlined.

use base64::Engine as _;
use serde_json::{Map, Value};

use super::nodes::Ctx;

pub(super) fn convert_image_fill(mut obj: Map<String, Value>, ctx: &mut Ctx<'_>) -> Option<Value> {
    let Some(Value::String(url)) = obj.remove("url") else {
        ctx.report.note("image fill without a file");
        return None;
    };
    let data_url = resolve(&url, ctx)?;
    obj.insert("url".to_owned(), Value::String(data_url));
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

fn resolve(url: &str, ctx: &mut Ctx<'_>) -> Option<String> {
    if url.starts_with("data:") || url.starts_with("http://") || url.starts_with("https://") {
        return Some(url.to_owned());
    }
    if let Some(cached) = ctx.images.get(url) {
        return cached.clone();
    }
    let relative = url.trim_start_matches("./");
    let resolved = if is_inside_document_folder(relative) {
        mime_type(relative).and_then(|mime| {
            let bytes = (ctx.assets)(relative)?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            Some(format!("data:{mime};base64,{encoded}"))
        })
    } else {
        None
    };
    if resolved.is_none() {
        ctx.report.note_missing_image(url);
    }
    ctx.images.insert(url.to_owned(), resolved.clone());
    resolved
}

/// A design file must not pull arbitrary files into the document: only
/// plain relative paths below the document's folder are read.
fn is_inside_document_folder(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.starts_with('/')
        && !relative.contains('\\')
        && !relative.contains(':')
        && relative
            .split('/')
            .all(|part| !part.is_empty() && part != "..")
}

fn mime_type(relative: &str) -> Option<&'static str> {
    let extension = relative.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
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
