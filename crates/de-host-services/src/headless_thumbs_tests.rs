//! Headless renders paint each document's own blur-up thumbnails (#469).
//!
//! An image with no bytes yet (an unfetched URL, a missing asset) draws its
//! document's thumbnail. The scene pages carry their document's table and
//! the renderers paint inside it, so a render never reads the editor
//! canvas's table or another thread's document.
//!
//! The checks count thumbnail draws with the native backend's per-thread
//! paint diagnostics rather than sampling pixels: an image node paints its
//! neutral placeholder fill over the thumbnail (`canvas_viewport_image`), so
//! the thumbnail is drawn but is not what the final pixels show.

use base64::Engine as _;
use de_editor_core::EditorState;
use jian_ops_schema::image_thumbs::{self, ImageThumbs};

/// Never fetched in tests: the image stays byte-less, so its thumbnail draws.
const SHARED_SRC: &str = "https://example.invalid/de-469-shared.png";

/// A small solid-colour JPEG (well under the 4 KiB thumbnail bound).
fn solid_jpeg(rgb: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(8, 8, image::Rgb(rgb));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("encode JPEG");
    assert!(bytes.len() <= image_thumbs::MAX_THUMB_BYTES);
    bytes
}

/// One image node naming [`SHARED_SRC`], with a thumbnail table when
/// `thumb` is set.
fn document_source(thumb: Option<[u8; 3]>) -> String {
    let table = thumb
        .map(|rgb| {
            let paint_id = jian_ops_schema::node::image_src::paint_image_id(SHARED_SRC);
            let encoded = base64::engine::general_purpose::STANDARD.encode(solid_jpeg(rgb));
            format!(r#","imageThumbs":{{"{paint_id}":"{encoded}"}}"#)
        })
        .unwrap_or_default();
    format!(
        r#"{{"version":"0.8.0","children":[{{"type":"image","id":"img","x":0,"y":0,"width":100,"height":100,"src":"{SHARED_SRC}"}}]{table}}}"#
    )
}

fn state(thumb: Option<[u8; 3]>) -> EditorState {
    let doc = jian_ops_schema::load_str(&document_source(thumb))
        .expect("fixture")
        .value;
    EditorState::from_document(doc)
}

/// Thumbnail draws `render` made on this thread.
fn thumb_draws(render: impl FnOnce()) -> u64 {
    de_host_native::begin_image_paint_diagnostics();
    render();
    de_host_native::end_image_paint_diagnostics().successful_thumbnail_draws
}

fn render_screenshot(state: &EditorState) -> Vec<u8> {
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    crate::export::render_node_raster_bytes(&scene, "img", crate::export::RasterFormat::Png, 1.0)
        .expect("render")
}

#[test]
fn a_headless_screenshot_draws_its_documents_own_blur_up() {
    let with_thumb = state(Some([220, 20, 20]));
    let without = state(None);
    // Whatever table the calling thread (a UI thread showing another
    // document) has scoped, the render uses the page's own: a document
    // with a thumbnail draws it, one without draws none.
    let drawn = image_thumbs::with_thread_table(&without.image_thumbs, || {
        thumb_draws(|| {
            render_screenshot(&with_thumb);
        })
    });
    assert!(drawn > 0, "the document's own thumbnail is drawn");
    let not_drawn = image_thumbs::with_thread_table(&with_thumb.image_thumbs, || {
        thumb_draws(|| {
            render_screenshot(&without);
        })
    });
    assert_eq!(not_drawn, 0, "another document's thumbnail is never drawn");
}

#[test]
fn page_and_pdf_exports_paint_without_the_process_wide_table() {
    let green = state(Some([20, 200, 20]));
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&green);
    let page = scene.active_page().expect("page");
    let raster = thumb_draws(|| {
        crate::export::render_page_raster_bytes(page, crate::export::RasterFormat::Png, 1.0)
            .expect("page render");
    });
    assert!(raster > 0, "page raster draws the thumbnail");
    let pdf = thumb_draws(|| {
        let bytes = crate::export::pdf::render_page_pdf_bytes(page).expect("PDF");
        assert!(bytes.starts_with(b"%PDF-"));
    });
    assert!(pdf > 0, "PDF draws the thumbnail");
    assert!(
        image_thumbs::thumb_for(jian_ops_schema::node::image_src::paint_image_id(SHARED_SRC))
            .is_none(),
        "rendering never publishes the document's table"
    );
}

fn file_path_screenshot(dir: &std::path::Path, file: &std::path::Path) -> Vec<u8> {
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "get_screenshot", "arguments": {
            "filePath": file.to_string_lossy(),
            "nodeId": "img",
        }}
    })
    .to_string();
    let response = crate::mcp_serve::file_path::process_message_for_file_path_arg(
        Some(&dir.join("server.de")),
        &line,
    )
    .expect("dispatch")
    .expect("handled as a call on the other file");
    let reply: serde_json::Value = serde_json::from_str(response.trim()).expect("JSON-RPC reply");
    let image_base64 = reply["result"]["content"]
        .as_array()
        .and_then(|content| content.iter().find_map(|item| item["data"].as_str()))
        .unwrap_or_else(|| panic!("image content in {response}"));
    base64::engine::general_purpose::STANDARD
        .decode(image_base64)
        .expect("base64 PNG")
}

#[test]
fn a_file_path_screenshot_uses_that_files_table() {
    let dir = std::env::temp_dir().join(format!(
        "de-469-file-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let with_thumb = dir.join("with-thumb.de");
    std::fs::write(&with_thumb, document_source(Some([220, 20, 20]))).expect("document");
    let without = dir.join("without.de");
    std::fs::write(&without, document_source(None)).expect("document");

    // The connection thread of the live desktop, with the canvas
    // document's table in scope either way.
    let empty_canvas = state(None);
    let drawn = image_thumbs::with_thread_table(&empty_canvas.image_thumbs, || {
        thumb_draws(|| {
            let png = file_path_screenshot(&dir, &with_thumb);
            assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        })
    });
    assert!(drawn > 0, "the other file's own thumbnail is drawn");
    let canvas = state(Some([20, 20, 220]));
    let not_drawn = image_thumbs::with_thread_table(&canvas.image_thumbs, || {
        thumb_draws(|| {
            file_path_screenshot(&dir, &without);
        })
    });
    assert_eq!(
        not_drawn, 0,
        "the canvas document's thumbnail is never drawn"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn concurrent_headless_renders_each_paint_their_own_document() {
    let threads: Vec<_> = (0..6u8)
        .map(|index| {
            std::thread::spawn(move || {
                let has_thumb = index % 2 == 0;
                let state = state(has_thumb.then_some([index * 30, 20, 220]));
                for round in 0..6 {
                    let drawn = thumb_draws(|| {
                        render_screenshot(&state);
                    });
                    assert_eq!(
                        drawn > 0,
                        has_thumb,
                        "thread {index} round {round}: draws only its own thumbnail"
                    );
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("render thread");
    }
}

#[test]
fn an_owned_table_outside_any_scope_is_not_what_painters_read() {
    // A table a document owns is read only inside a scope or through the
    // page; the process-wide fallback is a separate table.
    let owned = ImageThumbs::new();
    owned.store(1, vec![0xff, 0xd8, 0xff, 0xd9]);
    assert!(!image_thumbs::active().same_table(&owned));
}
