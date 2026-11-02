//! The folder relative browser pages resolve against (#283).

use super::{document_folder_of, load_editor_state_with_report, set_file_name_display};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "design-editor-doc-folder-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[test]
fn opening_a_file_records_its_folder() {
    let dir = temp_dir("open");
    let file = dir.join("design.de");
    std::fs::write(&file, r#"{"version":"1.0.0","children":[]}"#).expect("seed");
    let loaded = load_editor_state_with_report(&file, de_editor_core::Locale::EnUs).expect("load");
    assert_eq!(
        loaded.state.editor_ui.document_folder,
        document_folder_of(&file)
    );
    assert!(
        std::path::Path::new(loaded.state.editor_ui.document_folder.as_deref().unwrap())
            .is_absolute()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_elsewhere_moves_the_folder_and_unbinding_keeps_it() {
    let mut state = de_editor_core::EditorState::new();
    state.editor_ui.document_folder = Some("/imported/from".into());
    set_file_name_display(&mut state, None);
    assert_eq!(
        state.editor_ui.document_folder.as_deref(),
        Some("/imported/from")
    );
    let saved = std::path::Path::new("/saved/here/a.de");
    set_file_name_display(&mut state, Some(saved));
    // Absolute on this platform (`D:\saved\here` on Windows).
    assert_eq!(state.editor_ui.document_folder, document_folder_of(saved));
    assert_ne!(
        state.editor_ui.document_folder.as_deref(),
        Some("/imported/from")
    );
}

#[test]
fn a_bare_file_name_resolves_against_the_working_directory() {
    let folder = document_folder_of(std::path::Path::new("design.de")).expect("a folder");
    assert!(std::path::Path::new(&folder).is_absolute(), "{folder}");
}
