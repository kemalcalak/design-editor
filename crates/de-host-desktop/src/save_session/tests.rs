use super::*;

fn temp_document_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "design-editor-save-session-{tag}-{}-{}.de",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

#[test]
fn worker_saves_the_captured_revision_not_later_edits() {
    let path = temp_document_path("snapshot");
    let mut state = EditorState::new();
    state.doc.name = Some("captured".into());
    state.mark_document_changed();
    let revision = state.document_revision();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), true, None),
        EnqueueOutcome::Started
    );
    state.doc.name = Some("edited-after-save".into());
    state.mark_document_changed();

    let completion = session.wait_next().expect("save completion");
    assert!(completion.result.is_ok());
    assert_eq!(completion.revision, revision);
    let loaded = de_host_services::doc_io::load_editor_state(&path, de_editor_core::Locale::EnUs)
        .expect("load saved snapshot");
    assert_eq!(loaded.doc.name.as_deref(), Some("captured"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn identical_in_flight_request_is_not_duplicated() {
    let path = temp_document_path("dedupe");
    let state = EditorState::new();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::Started
    );
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::AlreadyPending
    );
    assert!(session.wait_next().expect("save completion").result.is_ok());
    assert!(!session.is_active());
    let _ = std::fs::remove_file(path);
}

#[test]
fn pending_identity_distinguishes_save_from_save_as() {
    let path = temp_document_path("operation-identity");
    let state = EditorState::new();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::Started
    );
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), true, None),
        EnqueueOutcome::Queued,
        "a shared save must not impersonate an exact Save-As request"
    );

    let shared = session.wait_next().expect("shared save completion");
    assert!(shared.result.is_ok());
    assert!(!shared.set_current_path);
    let save_as = session.wait_next().expect("Save-As completion");
    assert!(save_as.result.is_ok());
    assert!(save_as.set_current_path);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pending_target_is_scoped_to_the_document_epoch() {
    let path = temp_document_path("target-epoch");
    let state = EditorState::new();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 7, path.clone(), true, None),
        EnqueueOutcome::Started
    );
    assert_eq!(session.latest_target(7), Some(path.as_path()));
    assert_eq!(session.latest_target(8), None);
    assert!(session.wait_next().expect("save completion").result.is_ok());
    let _ = std::fs::remove_file(path);
}

#[test]
fn unchanged_bound_document_skips_snapshot_only_while_the_file_exists() {
    let path = temp_document_path("unchanged-skip");
    std::fs::write(&path, b"existing OP").expect("write bound file marker");
    let mut state = EditorState::new();
    state.mark_saved_revision();

    assert!(can_skip_unchanged_current_save(
        &state,
        Some(&path),
        &path,
        false
    ));
    assert!(!can_skip_unchanged_current_save(
        &state,
        Some(&path),
        &path,
        true
    ));
    state.mark_document_changed();
    assert!(!can_skip_unchanged_current_save(
        &state,
        Some(&path),
        &path,
        false
    ));
    std::fs::remove_file(&path).expect("remove bound file marker");
    state.mark_saved_revision();
    assert!(!can_skip_unchanged_current_save(
        &state,
        Some(&path),
        &path,
        false
    ));
}

#[test]
fn path_comparison_normalizes_equivalent_paths() {
    let source = temp_document_path("path-identity");
    std::fs::write(&source, b"existing OP").expect("write source");
    let dotted_alias = source
        .parent()
        .expect("source parent")
        .join(".")
        .join(source.file_name().expect("source file name"));
    assert!(!paths_provably_distinct(&source, &dotted_alias));

    let distinct = temp_document_path("path-identity-distinct");
    assert!(paths_provably_distinct(&source, &distinct));

    let unresolved = temp_document_path("missing-path-parent")
        .with_extension("")
        .join("copy.de");
    assert!(
        !paths_provably_distinct(&source, &unresolved),
        "an unresolved target must fail closed"
    );
    let _ = std::fs::remove_file(source);
}

#[cfg(unix)]
#[test]
fn path_comparison_rejects_symlink_alias() {
    let source = temp_document_path("path-symlink-source");
    let alias = temp_document_path("path-symlink-alias");
    std::fs::write(&source, b"existing OP").expect("write source");
    std::os::unix::fs::symlink(&source, &alias).expect("create source alias");

    assert!(!paths_provably_distinct(&source, &alias));

    let _ = std::fs::remove_file(alias);
    let _ = std::fs::remove_file(source);
}

#[test]
fn next_capture_reuses_only_the_live_in_flight_snapshot() {
    let path = temp_document_path("capture-anchor");
    let mut state = EditorState::new();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 7, path.clone(), false, None),
        EnqueueOutcome::Started
    );

    let first = session.running.as_ref().expect("running snapshot");
    assert!(std::ptr::eq(
        session
            .capture_anchor(7, state.document_generation())
            .unwrap(),
        first.snapshot.as_ref()
    ));
    assert!(session
        .capture_anchor(8, state.document_generation())
        .is_none());

    state.doc.name = Some("new revision".into());
    state.mark_document_changed();
    assert_eq!(
        session.enqueue(&state, 7, path.clone(), false, None),
        EnqueueOutcome::Queued
    );
    let queued = session.queued.as_ref().expect("queued snapshot");
    assert!(std::ptr::eq(
        session
            .capture_anchor(7, state.document_generation())
            .unwrap(),
        queued.snapshot.as_ref()
    ));

    while let Some(completion) = session.wait_next() {
        assert!(completion.result.is_ok());
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn queued_requests_coalesce_to_the_latest_revision_and_commit_in_order() {
    let path = temp_document_path("coalesce");
    let mut state = EditorState::new();
    state.doc.name = Some("first".into());
    state.mark_document_changed();
    let first_revision = state.document_revision();
    let mut session = SaveSession::new();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::Started
    );

    state.doc.name = Some("superseded".into());
    state.mark_document_changed();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::Queued
    );
    state.doc.name = Some("latest".into());
    state.mark_document_changed();
    let latest_revision = state.document_revision();
    assert_eq!(
        session.enqueue(&state, 0, path.clone(), false, None),
        EnqueueOutcome::Queued
    );

    let first = session.wait_next().expect("first save completion");
    assert!(first.result.is_ok());
    assert_eq!(first.revision, first_revision);
    let latest = session.wait_next().expect("latest save completion");
    assert!(latest.result.is_ok());
    assert_eq!(latest.revision, latest_revision);
    assert!(!session.is_active());

    let loaded = de_host_services::doc_io::load_editor_state(&path, de_editor_core::Locale::EnUs)
        .expect("load final saved snapshot");
    assert_eq!(loaded.doc.name.as_deref(), Some("latest"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn stale_epoch_ack_cannot_rebind_save_as_to_a_replaced_document() {
    let mut app = crate::DesktopApp::new(None);
    let old_epoch = app.host.document_epoch();
    let old_generation = app.host.editor_state().document_generation();
    let old_revision = app.host.editor_state().document_revision();
    app.host.replace_editor_state(EditorState::new());
    assert_ne!(app.host.document_epoch(), old_epoch);

    let applied = app.apply_save_completion(SaveCompletion {
        path: PathBuf::from("stale-save-as.de"),
        set_current_path: true,
        document_epoch: old_epoch,
        generation: old_generation,
        revision: old_revision,
        result: Ok(()),
    });

    assert!(applied);
    assert!(app.current_path.is_none());
    assert!(app
        .host
        .editor_state()
        .editor_ui
        .file_name_display
        .is_none());
}
