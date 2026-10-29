//! The native reader behind `de_mcp::local_image` (#268): an image fill or
//! image node written with a local file path is read here once and embedded
//! as a `data:` url, like every other image entry path.

use de_mcp::local_image::LocalImageError;
use std::path::Path;

/// Largest local image embedded from a write; the same 16 MB pen.dev's
/// image tools accept.
const MAX_LOCAL_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

/// Let MCP writes in this process read local image paths. Not for the web
/// daemon: a browser document must not pull in the daemon host's files.
pub fn install() {
    de_mcp::local_image::install_local_image_reader(read_local_image);
}

/// Read `url` (an absolute path or a `file://` url) as a `data:` url.
pub fn read_local_image(url: &str) -> Result<String, LocalImageError> {
    let path = file_path(url);
    let mime = de_pen_loader::image_mime_type(&path)
        .ok_or_else(|| LocalImageError::UnsupportedType { url: url.into() })?;
    let path = Path::new(&path);
    let unreadable = |e: std::io::Error| LocalImageError::Unreadable {
        url: url.into(),
        detail: e.to_string(),
    };
    let bytes = std::fs::metadata(path).map_err(unreadable)?.len();
    if bytes > MAX_LOCAL_IMAGE_BYTES {
        return Err(LocalImageError::TooLarge {
            url: url.into(),
            bytes,
            max: MAX_LOCAL_IMAGE_BYTES,
        });
    }
    let data = std::fs::read(path).map_err(unreadable)?;
    Ok(de_pen_loader::image_data_url(mime, &data))
}

/// The file system path of `url`: a plain path as given; a `file://` url
/// without its scheme and `localhost` host, percent-decoded (`%20` is a
/// space).
fn file_path(url: &str) -> String {
    let Some(rest) = url.strip_prefix("file://") else {
        return url.to_string();
    };
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let hex = |i: usize| bytes.get(i).and_then(|b| (*b as char).to_digit(16));
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(high), Some(low)) = (hex(i + 1), hex(i + 2)) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_png_path_as_a_data_url_and_names_a_missing_one() {
        let dir = std::env::temp_dir().join(format!("de-local-image-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let png = dir.join("logo.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G']).expect("write png");
        let url = png.to_str().expect("utf8 path");

        assert_eq!(
            read_local_image(url).expect("read"),
            "data:image/png;base64,iVBORw=="
        );
        assert_eq!(
            read_local_image(&format!("file://{url}")).expect("read file url"),
            "data:image/png;base64,iVBORw=="
        );
        let missing = dir.join("missing.png");
        let err = read_local_image(missing.to_str().expect("utf8")).expect_err("missing");
        assert!(matches!(err, LocalImageError::Unreadable { .. }), "{err}");
        let text = dir.join("notes.txt");
        let err = read_local_image(text.to_str().expect("utf8")).expect_err("not an image");
        assert!(
            matches!(err, LocalImageError::UnsupportedType { .. }),
            "{err}"
        );
        let spaced = dir.join("My Shots");
        std::fs::create_dir_all(&spaced).expect("spaced dir");
        std::fs::write(spaced.join("hero.png"), [0x89, b'P', b'N', b'G']).expect("write");
        let encoded = format!("file://localhost{}/My%20Shots/hero.png", dir.display());
        assert_eq!(
            read_local_image(&encoded).expect("decoded"),
            "data:image/png;base64,iVBORw=="
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
