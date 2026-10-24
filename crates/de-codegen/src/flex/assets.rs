//! Images written next to the exported HTML.
//!
//! An embedded `data:` image becomes a file under the asset directory and the
//! markup references it relatively, the way pen.dev writes its exports. With
//! extraction off (the browser download, which is one file) the `data:` URI
//! stays inline. Remote URLs and paths pass through unchanged: there are no
//! bytes to copy.

use base64::Engine as _;

/// One file to write beside the HTML: `path` is relative to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlAsset {
    pub path: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

/// Collects the image files one export writes, de-duplicated by source.
pub(crate) struct Assets {
    dir: Option<String>,
    files: Vec<HtmlAsset>,
    by_src: Vec<(String, String)>,
}

impl Assets {
    /// `dir` is the relative asset directory; `None` keeps sources inline.
    pub fn new(dir: Option<String>) -> Self {
        Self {
            dir: dir.map(|d| d.trim_matches('/').to_string()),
            files: Vec::new(),
            by_src: Vec::new(),
        }
    }

    /// The URL markup should use for `src`. `hint` names the file (a layer
    /// name slug).
    pub fn url(&mut self, src: &str, hint: &str) -> String {
        let src = src.trim();
        let Some(dir) = self.dir.as_deref() else {
            return src.to_string();
        };
        if let Some((_, path)) = self.by_src.iter().find(|(s, _)| s == src) {
            return path.clone();
        }
        let Some((mime, bytes)) = decode_data_uri(src) else {
            return src.to_string();
        };
        let stem = if hint.is_empty() { "image" } else { hint };
        let ext = extension(&mime);
        let mut path = format!("{dir}/{stem}.{ext}");
        let mut n = 2;
        while self.files.iter().any(|f| f.path == path) {
            path = format!("{dir}/{stem}-{n}.{ext}");
            n += 1;
        }
        self.files.push(HtmlAsset {
            path: path.clone(),
            mime_type: mime,
            bytes,
        });
        self.by_src.push((src.to_string(), path.clone()));
        path
    }

    pub fn into_files(self) -> Vec<HtmlAsset> {
        self.files
    }
}

/// `(mime, bytes)` of a base64 `data:` URI. Percent-encoded (non-base64)
/// payloads are rare for images and stay inline.
fn decode_data_uri(src: &str) -> Option<(String, Vec<u8>)> {
    let rest = src.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    let mime = if mime.is_empty() {
        "application/octet-stream"
    } else {
        mime
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    Some((mime.to_ascii_lowercase(), bytes))
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

    #[test]
    fn data_uris_become_relative_files_once() {
        let mut assets = Assets::new(Some("assets".into()));
        assert_eq!(assets.url(PNG, "hero"), "assets/hero.png");
        assert_eq!(assets.url(PNG, "other"), "assets/hero.png");
        assert_eq!(
            assets.url("data:image/jpeg;base64,/9j/", "hero"),
            "assets/hero.jpg"
        );
        assert_eq!(assets.url("https://x.test/a.png", "a"), "https://x.test/a.png");
        let files = assets.into_files();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].bytes[..4], [0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn inline_mode_keeps_the_source() {
        let mut assets = Assets::new(None);
        assert_eq!(assets.url(PNG, "hero"), PNG);
        assert!(assets.into_files().is_empty());
    }

    #[test]
    fn duplicate_stems_get_a_counter() {
        let mut assets = Assets::new(Some("assets/".into()));
        assets.url(PNG, "img");
        assert_eq!(
            assets.url("data:image/png;base64,AAAA", "img"),
            "assets/img-2.png"
        );
    }
}
