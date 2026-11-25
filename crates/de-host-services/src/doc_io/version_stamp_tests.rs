//! `.de` `version` names the release that last wrote the file (#408).

use super::{load_editor_state, save_snapshot_to_path, save_to_path, CanonicalSaveSnapshot};
use de_editor_core::document_version::PRODUCT_VERSION;
use de_editor_core::Locale;
use std::path::{Path, PathBuf};

fn temp_document_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "design-editor-version-{tag}-{}-{}.de",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn saved_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("saved document"))
        .expect("saved document json")
}

fn write_source(tag: &str, source: &str) -> PathBuf {
    let path = temp_document_path(tag);
    std::fs::write(&path, source).expect("write source");
    path
}

#[test]
fn a_file_from_an_older_release_is_saved_with_this_release() {
    let path = write_source(
        "older",
        r#"{"version":"0.5.5","children":[{"type":"rectangle","id":"r","width":10,"height":10}]}"#,
    );
    let state = load_editor_state(&path, Locale::EnUs).expect("load");
    save_to_path(&state, &path).expect("save");

    let saved = saved_json(&path);
    assert_eq!(saved["version"], PRODUCT_VERSION);
    assert!(saved.get("formatVersion").is_none());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_background_save_writes_this_release_too() {
    let path = write_source("snapshot", r#"{"version":"0.5.5","children":[]}"#);
    let state = load_editor_state(&path, Locale::EnUs).expect("load");
    save_snapshot_to_path(&CanonicalSaveSnapshot::capture(&state), &path).expect("save");

    assert_eq!(saved_json(&path)["version"], PRODUCT_VERSION);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_legacy_schema_marker_survives_as_format_version() {
    let path = write_source("legacy", r#"{"version":"1.1","children":[]}"#);
    let state = load_editor_state(&path, Locale::EnUs).expect("load");
    save_to_path(&state, &path).expect("save");

    let saved = saved_json(&path);
    assert_eq!(saved["version"], PRODUCT_VERSION);
    assert_eq!(saved["formatVersion"], "1.1");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn opening_does_not_mark_the_document_edited() {
    let path = write_source("clean", r#"{"version":"0.5.5","children":[]}"#);
    let state = load_editor_state(&path, Locale::EnUs).expect("load");
    assert!(!state.is_dirty());
    let _ = std::fs::remove_file(&path);
}
