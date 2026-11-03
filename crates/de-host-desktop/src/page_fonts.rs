//! A page's own `@font-face` files (#301): what MCP `browser`
//! import-to-canvas registers when a family the import uses is not on
//! Google Fonts. The page served them, so their license is unknown: they
//! are registered with `jian-skia` for this session and never written to
//! the user's [`crate::fonts::FontStore`].

use de_host_services::font_host::PageFaceError;

use crate::fonts::MAX_FONT_BYTES;

/// The sfnt (TTF / OTF) bytes of a web font file: WOFF and WOFF2 are
/// decoded, anything else passes through for skia to judge. `None` for a
/// file that is too large or does not decode.
pub(crate) fn decode_web_font(bytes: Vec<u8>) -> Option<Vec<u8>> {
    if bytes.len() > MAX_FONT_BYTES {
        return None;
    }
    let sfnt = match bytes.get(..4) {
        Some(b"wOFF") => wuff::decompress_woff1(&bytes).ok()?,
        Some(b"wOF2") => wuff::decompress_woff2(&bytes).ok()?,
        _ => bytes,
    };
    (sfnt.len() <= MAX_FONT_BYTES).then_some(sfnt)
}

/// Register every readable face among `files` and name the family they
/// paint under: the face whose own family matches `family`, else the
/// first one registered.
pub(crate) fn register_page_faces(
    family: &str,
    files: Vec<Vec<u8>>,
) -> Result<String, PageFaceError> {
    let count = files.len();
    let mut registered: Vec<String> = Vec::new();
    for file in files {
        let Some(sfnt) = decode_web_font(file) else {
            continue;
        };
        if let Ok(blob) = jian_skia::register_imported_font(sfnt) {
            registered.push(blob.family);
        }
    }
    registered
        .iter()
        .find(|name| name.eq_ignore_ascii_case(family))
        .or_else(|| registered.first())
        .cloned()
        .ok_or(PageFaceError::Unreadable { files: count })
}

// Gated off Windows like `fonts.rs`: skia's DirectWrite font manager is not
// safe to drive from cargo's parallel test threads there.
#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    #[test]
    fn plain_sfnt_passes_through_and_junk_is_refused() {
        let ttf = b"\0\x01\0\0rest".to_vec();
        assert_eq!(decode_web_font(ttf.clone()), Some(ttf));
        assert_eq!(decode_web_font(b"wOF2 not really".to_vec()), None);
        assert_eq!(decode_web_font(vec![0; MAX_FONT_BYTES + 1]), None);
    }

    #[test]
    fn unreadable_files_are_an_error_naming_how_many() {
        let error = register_page_faces("Nope", vec![b"junk".to_vec(), Vec::new()])
            .expect_err("nothing registers");
        assert_eq!(error, PageFaceError::Unreadable { files: 2 });
    }

    #[test]
    fn a_bundled_face_registers_under_its_own_family() {
        let bytes = include_bytes!("../assets/fonts/DMMono-Medium.ttf").to_vec();
        let family = register_page_faces("DM Mono Web", vec![bytes]).expect("registers");
        assert_eq!(family, "DM Mono");
    }
}
