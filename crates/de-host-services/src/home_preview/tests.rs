use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::*;

const TALL_DOC: &str = r##"{"version":"1.0.0","children":[{"id":"tall","type":"rectangle","x":0,"y":0,"width":1440,"height":4000,"fill":[{"type":"solid","color":"#123456"}]}]}"##;
const EMPTY_DOC: &str = r#"{"version":"1.0.0","children":[]}"#;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-home-preview-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// PNG IHDR: width then height, big-endian, from byte 16.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height)
}

fn write_doc(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write doc");
    path
}

#[test]
fn a_document_renders_to_a_capped_overview() {
    let dir = scratch("capped");
    let doc = write_doc(&dir, "tall.op", TALL_DOC);
    let bytes = render_document_png(&doc).expect("renders");
    let (width, height) = png_size(&bytes);
    assert_eq!(height, PREVIEW_MAX_EDGE as u32, "long edge is capped");
    assert_eq!(
        width,
        (1440.0 * PREVIEW_MAX_EDGE / 4000.0_f32).round() as u32
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_empty_document_gets_a_blank_preview() {
    let dir = scratch("empty");
    let doc = write_doc(&dir, "empty.op", EMPTY_DOC);
    let bytes = render_document_png(&doc).expect("an empty page still renders");
    assert_eq!(png_size(&bytes), (1, 1));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_preview_is_stale_until_written_and_again_after_the_document_changes() {
    let dir = scratch("stale");
    let doc = write_doc(&dir, "doc.op", EMPTY_DOC);
    let preview = preview_path(&dir.join("thumbs"), &doc);
    assert!(is_stale(&doc, &preview), "no preview yet");

    write_preview(&preview, b"png", modified_time(&doc)).expect("written");
    assert!(
        !is_stale(&doc, &preview),
        "stamped with the document's time"
    );
    assert_eq!(modified_time(&preview), modified_time(&doc));

    let later = SystemTime::now() + Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&doc)
        .and_then(|file| file.set_modified(later))
        .expect("touch doc");
    assert!(is_stale(&doc, &preview), "the document changed outside");

    let older = SystemTime::now() - Duration::from_secs(3600);
    write_preview(&preview, b"png", Some(later)).expect("rewritten");
    std::fs::File::options()
        .write(true)
        .open(&doc)
        .and_then(|file| file.set_modified(older))
        .expect("restore an older copy");
    assert!(is_stale(&doc, &preview), "an older copy restored over it");

    assert!(
        !is_stale(&dir.join("missing.op"), &preview),
        "a missing document needs nothing"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_worker_renders_a_saved_file_on_demand() {
    let dir = scratch("worker");
    let doc = write_doc(&dir, "doc.op", TALL_DOC);
    let thumbs = dir.join("thumbs");
    let before = written_count();
    queue_into(&thumbs, &doc);
    assert!(wait_idle(Duration::from_secs(30)), "worker finished");
    let preview = preview_path(&thumbs, &doc);
    let bytes = std::fs::read(&preview).expect("preview written");
    assert_eq!(png_size(&bytes).1, PREVIEW_MAX_EDGE as u32);
    assert!(!is_stale(&doc, &preview));
    assert!(written_count() > before);
    let _ = std::fs::remove_dir_all(dir);
}
