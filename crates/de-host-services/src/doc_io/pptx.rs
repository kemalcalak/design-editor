//! Opening a PowerPoint deck (#246): the `.pptx` converts into a new
//! untitled deck document, one board per slide, the way a pen.dev `.pen`
//! opens. The source is never written; the first save asks for a `.de`.

use super::load::{finish_loaded_state, localized_with_name};
use super::{DocIoError, DocumentLoadReport, LoadedEditorState};
use de_editor_core::EditorState;

pub(super) fn load_pptx(
    path: &std::path::Path,
    locale: de_editor_core::Locale,
) -> Result<LoadedEditorState, DocIoError> {
    let bytes = std::fs::read(path).map_err(|e| DocIoError::Io(e.to_string()))?;
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let import = de_pptx::import_pptx(&bytes, &name).map_err(|error| {
        DocIoError::NotPptxDeck(
            localized_with_name(locale, "dialog.loadErrorNotPptx", path)
                .replace("{{detail}}", &error.to_string()),
        )
    })?;
    let mut state = EditorState::from_document(import.document);
    state.tag_as_deck();
    Ok(LoadedEditorState {
        state: finish_loaded_state(state, None),
        report: DocumentLoadReport {
            pptx_import: Some(import.warnings),
            ..DocumentLoadReport::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::scene_template_catalog::TemplateScene;

    #[test]
    fn a_deck_opens_as_an_untitled_slides_document() {
        // A deck written by the editor's own exporter, opened back.
        let source = jian_ops_schema::load_str(
            r##"{"version":"1.0.0","children":[
                {"type":"frame","id":"a","x":0,"y":0,"width":960,"height":540,"layout":"none",
                 "children":[{"type":"text","id":"t","x":40,"y":40,"width":400,"height":60,
                              "content":"Hello deck","fontSize":32}]},
                {"type":"frame","id":"b","x":1100,"y":0,"width":960,"height":540,"layout":"none"}
            ]}"##,
        )
        .expect("fixture")
        .value;
        let (bytes, _) = crate::export_pptx::build_deck_pptx(&EditorState::from_document(source))
            .expect("export");
        let dir = std::env::temp_dir().join(format!("de-doc-io-pptx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("Deck.pptx");
        std::fs::write(&path, bytes).expect("write");

        let loaded =
            crate::doc_io::load_editor_state_with_report(&path, de_editor_core::Locale::EnUs)
                .expect("opens");
        assert!(loaded.report.is_import());
        assert_eq!(loaded.state.editor_ui.scenario, Some(TemplateScene::Slides));
        let boards = de_editor_core::preview_slideshow::active_page_boards(&loaded.state);
        assert_eq!(boards.len(), 2);

        // A save aimed at the deck (an MCP `filePath`) is refused.
        let before = std::fs::read(&path).expect("deck bytes");
        let err = crate::doc_io::save_to_path(&loaded.state, &path).expect_err("source is kept");
        assert!(
            matches!(err, DocIoError::PptxSourceReadOnly { .. }),
            "{err}"
        );
        assert_eq!(std::fs::read(&path).expect("deck bytes"), before);

        std::fs::write(&path, b"not a zip").expect("write");
        let Err(error) = crate::doc_io::load_editor_state(&path, de_editor_core::Locale::EnUs)
        else {
            panic!("garbage must not open");
        };
        assert!(matches!(error, DocIoError::NotPptxDeck(_)));
        assert!(error.to_string().contains("Deck.pptx"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Open time of a realistic deck on the desktop path (#246): 50 slides,
/// five distinct ~1 MB photos shared across them, title + body text and a
/// few shapes per slide. Run with
/// `cargo test -p de-host-services --release open_time -- --ignored --nocapture`.
#[cfg(test)]
mod timing {
    use de_editor_core::EditorState;

    fn photo(seed: u32) -> String {
        use base64::Engine;
        let mut state = seed.wrapping_mul(2_654_435_761);
        let image = image::RgbImage::from_fn(900, 600, |x, y| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let noise = (state & 0x3f) as u8;
            image::Rgb([
                (x / 4) as u8 ^ noise,
                (y / 3) as u8 ^ noise,
                noise.wrapping_mul(3),
            ])
        });
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode");
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    }

    #[test]
    #[ignore = "timing probe; run in release"]
    fn open_time_of_a_fifty_slide_deck() {
        let photos: Vec<String> = (0..5).map(photo).collect();
        let boards: Vec<serde_json::Value> = (0..50)
            .map(|i| {
                serde_json::json!({
                    "type": "frame", "id": format!("b{i}"), "x": i * 2000, "y": 0,
                    "width": 1920, "height": 1080, "layout": "none",
                    "fill": [{"type": "solid", "color": "#FFFFFF"}],
                    "children": [
                        {"type": "text", "id": format!("t{i}"), "x": 120, "y": 80, "width": 1600,
                         "height": 120, "content": format!("Slide {i}: quarterly results"), "fontSize": 72},
                        {"type": "text", "id": format!("p{i}"), "x": 120, "y": 260, "width": 800,
                         "height": 600, "content": "First point\nSecond point\nThird point\nFourth point",
                         "fontSize": 36},
                        {"type": "image", "id": format!("i{i}"), "x": 1000, "y": 260, "width": 800,
                         "height": 533, "src": photos[i % photos.len()]},
                        {"type": "rectangle", "id": format!("r{i}"), "x": 0, "y": 1040, "width": 1920,
                         "height": 40, "fill": [{"type": "solid", "color": "#4472C4"}]},
                        {"type": "ellipse", "id": format!("e{i}"), "x": 1800, "y": 40, "width": 60,
                         "height": 60, "fill": [{"type": "solid", "color": "#ED7D31"}]}
                    ]
                })
            })
            .collect();
        let source: jian_ops_schema::PenDocument =
            serde_json::from_value(serde_json::json!({"version": "1.0.0", "children": boards}))
                .expect("fixture");
        let (bytes, _) = crate::export_pptx::build_deck_pptx(&EditorState::from_document(source))
            .expect("export");
        let dir = std::env::temp_dir().join(format!("de-pptx-timing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("Deck.pptx");
        std::fs::write(&path, &bytes).expect("write");

        let started = std::time::Instant::now();
        let loaded =
            crate::doc_io::load_editor_state_with_report(&path, de_editor_core::Locale::EnUs)
                .expect("opens");
        let parsed = started.elapsed();
        // The desktop open frames the document right after, which builds
        // the layout scene once.
        let _scene = de_pen_loader::editor_state_to_active_page_layout_scene(&loaded.state);
        let total = started.elapsed();
        println!(
            "deck {:.1} MB, {} boards: import {parsed:?}, import + layout scene {total:?}",
            bytes.len() as f64 / 1_048_576.0,
            de_editor_core::preview_slideshow::active_page_boards(&loaded.state).len()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
