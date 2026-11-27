//! Per-document image thumbnails on `EditorState` (#469).

use super::EditorState;
use jian_ops_schema::image_thumbs::{self, ImageThumbs};

fn parsed_with_thumb(id: u64) -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(&format!(
        r#"{{"version":"0.8.0","imageThumbs":{{"{id}":"/9j/2Q=="}},"children":[]}}"#
    ))
    .expect("document with an imageThumbs table")
    .value
}

#[test]
fn each_state_owns_the_table_its_document_was_parsed_with() {
    let first = EditorState::from_document(parsed_with_thumb(9_100_001));
    let second = EditorState::from_document(parsed_with_thumb(9_100_002));
    assert!(first.image_thumbs.contains(9_100_001));
    assert!(!first.image_thumbs.contains(9_100_002));
    assert!(second.image_thumbs.contains(9_100_002));
    assert!(!second.image_thumbs.contains(9_100_001));
    assert!(
        image_thumbs::thumb_for(9_100_001).is_none()
            && image_thumbs::thumb_for(9_100_002).is_none(),
        "building a state never publishes its table"
    );
}

#[test]
fn a_document_parsed_without_a_table_starts_empty() {
    let parsed = jian_ops_schema::load_str(r#"{"version":"0.8.0","children":[]}"#)
        .expect("plain document")
        .value;
    let state = EditorState::from_document(parsed);
    assert_eq!(state.image_thumbs.usage(), (0, 0));
}

#[test]
fn replacements_bring_their_own_table_and_absence_keeps_the_current_one() {
    let mut state = EditorState::from_document(parsed_with_thumb(9_100_003));
    let request_snapshot = state.clone();
    assert!(request_snapshot
        .image_thumbs
        .same_table(&state.image_thumbs));

    state.replace_document(parsed_with_thumb(9_100_004));
    assert!(state.image_thumbs.contains(9_100_004));
    assert!(!state.image_thumbs.contains(9_100_003));
    assert!(
        request_snapshot.image_thumbs.contains(9_100_003)
            && !request_snapshot.image_thumbs.contains(9_100_004),
        "a clone of the old document keeps the old table"
    );

    state.replace_document_with_undo(parsed_with_thumb(9_100_005));
    assert!(state.image_thumbs.contains(9_100_005));
    assert!(!state.image_thumbs.contains(9_100_004));

    let kept = state.image_thumbs.clone();
    let plain = jian_ops_schema::load_str(r#"{"version":"0.8.0","children":[]}"#)
        .expect("plain replacement")
        .value;
    state.replace_document(plain);
    assert!(state.image_thumbs.same_table(&kept));
}

#[test]
fn opened_documents_keep_their_table_through_the_version_stamp() {
    // An older release's file: the stamp replaces the `version` buffer the
    // pending seed is keyed on (#458), so the table is taken first.
    let parsed = jian_ops_schema::load_str(
        r#"{"version":"0.5.5","imageThumbs":{"9100006":"/9j/2Q=="},"children":[]}"#,
    )
    .expect("older release document")
    .value;
    let state = EditorState::from_opened_document(parsed);
    assert!(state.image_thumbs.contains(9_100_006));
}

#[test]
fn hollow_and_new_states_have_tables_of_their_own() {
    let a = EditorState::new();
    let b = EditorState::hollow();
    assert!(!a.image_thumbs.same_table(&b.image_thumbs));
    assert!(!a.image_thumbs.same_table(&ImageThumbs::new()));
}
