//! Opening a `.de` registers its `imageThumbs` whatever its version fields
//! say, and saving writes them back (#458). Legacy-schema files (no
//! `formatVersion`, `version` used as the schema marker) and files an older
//! release wrote used to open without them, because the re-stamp of
//! `version` dropped the parsed thumbnail seed.

use super::{load_editor_state, save_single_file_to_path, save_to_path};
use base64::Engine as _;
use de_editor_core::document_version::PRODUCT_VERSION;
use jian_ops_schema::image_thumbs;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A minimal JPEG (SOI, EOI).
const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xd9];

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-thumb-load-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// How the fixture keeps its thumbnail.
#[derive(Clone, Copy)]
enum ThumbForm {
    /// Inline base64, as a single-file save writes it.
    Inline,
    /// A `<stem>.assets/<name>.jpg` path, as a folder-layout save writes it.
    Folder,
}

/// One image node and its thumbnail, under `versions` (the raw version
/// fields). Returns the document path and the image's paint id.
fn write_fixture(dir: &Path, stem: &str, versions: &str, form: ThumbForm) -> (PathBuf, u64) {
    let src = format!("https://example.com/{stem}.png");
    let paint_id = jian_ops_schema::node::image_src::paint_image_id(&src);
    let thumb = match form {
        ThumbForm::Inline => base64::engine::general_purpose::STANDARD.encode(JPEG),
        ThumbForm::Folder => {
            let folder = dir.join(format!("{stem}.assets"));
            std::fs::create_dir_all(&folder).expect("asset folder");
            std::fs::write(folder.join("thumb.jpg"), JPEG).expect("thumbnail file");
            format!("{stem}.assets/thumb.jpg")
        }
    };
    let source = format!(
        r#"{{{versions},"children":[{{"type":"image","id":"img","x":0,"y":0,"width":10,"height":10,"src":"{src}"}}],"imageThumbs":{{"{paint_id}":"{thumb}"}}}}"#
    );
    let path = dir.join(format!("{stem}.de"));
    std::fs::write(&path, source).expect("fixture");
    (path, paint_id)
}

/// Open `path` on an empty registry and check its thumbnail is the one
/// painted.
fn open_and_check(path: &Path, paint_id: u64, label: &str) -> de_editor_core::EditorState {
    image_thumbs::clear_registry();
    let state = load_editor_state(path, de_editor_core::Locale::EnUs).expect(label);
    assert_eq!(
        image_thumbs::thumb_for(paint_id).as_deref(),
        Some(JPEG),
        "{label}: thumbnail registered on open"
    );
    state
}

fn saved_thumb(path: &Path, paint_id: u64) -> String {
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("saved file")).expect("JSON");
    saved["imageThumbs"][paint_id.to_string()]
        .as_str()
        .unwrap_or_else(|| panic!("{}: thumbnail kept on save: {saved}", path.display()))
        .to_owned()
}

/// Open, save both ways, reopen each save: the thumbnail is registered every
/// time and written in the form the save uses.
fn check_round_trip(tag: &str, versions: &str, form: ThumbForm) {
    let _guard = crate::test_support::lock_thumbnail_registry();
    let dir = temp_dir(tag);
    let (path, paint_id) = write_fixture(&dir, tag, versions, form);
    let state = open_and_check(&path, paint_id, &format!("{tag}: open"));

    let single = dir.join("single.de");
    save_single_file_to_path(&state, &single).expect("single-file save");
    assert_eq!(
        saved_thumb(&single, paint_id),
        base64::engine::general_purpose::STANDARD.encode(JPEG),
        "{tag}: single-file save inlines the thumbnail"
    );

    let folder = dir.join("folder.de");
    save_to_path(&state, &folder).expect("folder save");
    let thumb_path = saved_thumb(&folder, paint_id);
    assert!(
        thumb_path.starts_with("folder.assets/") && thumb_path.ends_with(".jpg"),
        "{tag}: folder save writes a file: {thumb_path}"
    );
    assert_eq!(
        std::fs::read(dir.join(&thumb_path)).expect("thumbnail file"),
        JPEG
    );

    open_and_check(&single, paint_id, &format!("{tag}: reopen single file"));
    open_and_check(&folder, paint_id, &format!("{tag}: reopen folder layout"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn legacy_schema_inline_thumbnails_load_and_save() {
    check_round_trip("legacy-inline", r#""version":"1.0.0""#, ThumbForm::Inline);
}

#[test]
fn legacy_schema_folder_thumbnails_load_and_save() {
    check_round_trip("legacy-folder", r#""version":"1.0.0""#, ThumbForm::Folder);
}

/// An unsupported legacy major takes the loader's best-effort retry, which
/// rewrites `version` to "1.0" before parsing.
#[test]
fn legacy_major_retried_as_v1_keeps_its_thumbnails() {
    check_round_trip("legacy-major", r#""version":"2.8""#, ThumbForm::Inline);
}

#[test]
fn older_release_current_schema_keeps_its_thumbnails() {
    check_round_trip(
        "older-release",
        r#""version":"0.5.5","formatVersion":"1.0""#,
        ThumbForm::Folder,
    );
}

#[test]
fn current_format_thumbnails_still_load_and_save() {
    let versions = format!(r#""version":"{PRODUCT_VERSION}","formatVersion":"1.0""#);
    check_round_trip("current-inline", &versions, ThumbForm::Inline);
    check_round_trip("current-folder", &versions, ThumbForm::Folder);
}
