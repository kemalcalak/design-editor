//! Tests for `library_files.rs`.

use super::*;
use de_editor_core::page_mutators::COMPONENTS_PAGE_NAME;
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::LibraryStatus;

/// A fresh, empty folder under the system temp dir.
fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("de-lib-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn library_json(label: &str) -> String {
    serde_json::json!({
        "version": "1.0",
        "variables": { "--brand": { "type": "color", "value": "#112233" } },
        "children": [{
            "id": "lib-btn", "type": "frame", "name": "Button", "reusable": true,
            "width": 120, "height": 40,
            "children": [{ "id": "lib-btn-t", "type": "text", "content": label }],
        }],
    })
    .to_string()
}

fn master_label(state: &EditorState) -> String {
    let page = state
        .doc
        .pages
        .as_ref()
        .unwrap()
        .iter()
        .find(|p| p.name == COMPONENTS_PAGE_NAME)
        .unwrap();
    let master = page
        .children
        .iter()
        .find(|n| n.id_str() == "lib-btn")
        .unwrap();
    serde_json::to_string(&master.children().unwrap()[0]).unwrap()
}

/// An absolute path under a fixed root on every OS (`/work/…` is not
/// absolute on Windows). Nothing is created on disk.
fn w(relative: &str) -> PathBuf {
    std::env::temp_dir().join("de-lib-paths").join(relative)
}

fn s(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn relative_links_round_trip_through_the_document_folder() {
    let base = w("designs");
    assert_eq!(
        library_link_path(Some(&base), &w("kits/ui.de")),
        "../kits/ui.de"
    );
    assert_eq!(
        library_link_path(Some(&base), &w("designs/lib/a.de")),
        "lib/a.de"
    );
    assert_eq!(
        resolve_library_path(Some(&base), "../kits/ui.de"),
        base.join("../kits/ui.de")
    );
    assert_eq!(library_link_path(None, &w("y.de")), s(w("y.de")));
}

#[test]
fn a_linked_file_syncs_and_picks_up_changes_on_disk() {
    let dir = temp_dir("sync");
    std::fs::write(dir.join("kit.de"), library_json("Old")).unwrap();
    let mut state = EditorState::new();
    rebase_library_links(&mut state, Some(&dir.join("doc.de")));
    state.apply_library_command(library_command(
        state
            .editor_ui
            .libraries
            .base_dir
            .clone()
            .map(PathBuf::from)
            .as_deref(),
        "kit.de",
        None,
    ));
    assert!(master_label(&state).contains("Old"));
    assert!(state
        .doc
        .variables
        .as_ref()
        .unwrap()
        .contains_key("--brand"));

    std::fs::write(dir.join("kit.de"), library_json("New")).unwrap();
    assert!(sync_libraries(&mut state).is_empty());
    assert!(master_label(&state).contains("New"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_library_is_reported_not_fatal() {
    let dir = temp_dir("missing");
    std::fs::write(dir.join("kit.de"), library_json("Kept")).unwrap();
    let mut state = EditorState::new();
    rebase_library_links(&mut state, Some(&dir.join("doc.de")));
    state.apply_library_command(library_command(Some(&dir), "kit.de", None));
    std::fs::remove_file(dir.join("kit.de")).unwrap();
    assert_eq!(sync_libraries(&mut state), vec!["kit.de".to_string()]);
    assert_eq!(
        state.editor_ui.libraries.entries[0].status,
        LibraryStatus::Missing
    );
    assert!(master_label(&state).contains("Kept"), "cached master stays");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_broken_library_reports_why() {
    let dir = temp_dir("broken");
    std::fs::write(dir.join("kit.de"), "not json").unwrap();
    let command = library_command(Some(&dir), "kit.de", None);
    assert!(matches!(
        command,
        LibraryCommand::Unavailable {
            status: LibraryStatus::Invalid(_),
            ..
        }
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_as_rebases_relative_links() {
    let mut state = EditorState::new();
    rebase_library_links(&mut state, Some(&w("a/doc.de")));
    state
        .editor_ui
        .libraries
        .set_links(vec![de_editor_core::LibraryLink::new("kit.de", None)]);
    rebase_library_links(&mut state, Some(&w("b/c/doc.de")));
    assert_eq!(state.editor_ui.libraries.links()[0].path, "../../a/kit.de");
    assert_eq!(state.editor_ui.libraries.base_dir, Some(s(w("b/c"))));
}

#[test]
fn links_made_before_the_first_save_become_relative() {
    let mut state = EditorState::new();
    state
        .editor_ui
        .libraries
        .set_links(vec![de_editor_core::LibraryLink::new(
            s(w("kits/ui.de")),
            None,
        )]);
    rebase_library_links(&mut state, Some(&w("designs/doc.de")));
    assert_eq!(state.editor_ui.libraries.links()[0].path, "../kits/ui.de");
}

#[test]
fn links_persist_in_editor_meta() {
    let mut state = EditorState::new();
    state
        .editor_ui
        .libraries
        .set_links(vec![de_editor_core::LibraryLink::new(
            "kit.de",
            Some("ui".into()),
        )]);
    let meta = crate::EditorMeta::from_state(&state);
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["libraries"][0]["path"], "kit.de");
    let src =
        serde_json::json!({ "version": "1.0", "children": [], "editorMeta": json }).to_string();
    let back = crate::extract_editor_meta(&src).unwrap();
    assert_eq!(back.libraries, meta.libraries);
}

#[test]
fn a_save_as_writes_links_relative_to_its_target() {
    let mut state = EditorState::new();
    rebase_library_links(&mut state, Some(&w("a/doc.de")));
    state
        .editor_ui
        .libraries
        .set_links(vec![de_editor_core::LibraryLink::new("kit.de", None)]);
    let meta = crate::EditorMeta::for_document(&state, &w("b/copy.de"));
    assert_eq!(meta.libraries[0].path, "../a/kit.de");
    // The live state still points at the folder it was opened from.
    assert_eq!(state.editor_ui.libraries.links()[0].path, "kit.de");
    assert_eq!(
        links_for_document(&state, None)[0].path,
        s(w("a/kit.de")),
        "no target folder: absolute"
    );
}

#[test]
fn metadata_without_links_keeps_the_current_ones() {
    let mut state = EditorState::new();
    state
        .editor_ui
        .libraries
        .set_links(vec![de_editor_core::LibraryLink::new("kit.de", None)]);
    crate::apply_editor_meta(&mut state, crate::EditorMeta::default());
    assert_eq!(state.editor_ui.libraries.entries.len(), 1);
}
