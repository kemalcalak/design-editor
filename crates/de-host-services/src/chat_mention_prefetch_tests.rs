use super::*;
use std::fs;
use std::time::Duration;

use de_editor_core::chat_mentions::ChatMention;

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-mention-read-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn file(rel: &str) -> ChatMention {
    ChatMention {
        kind: MentionKind::File,
        key: rel.into(),
        label: rel.into(),
    }
}

fn settle(reader: &mut MentionFileReader, state: &mut EditorState) {
    for _ in 0..500 {
        reader.sync(state);
        if !reader.is_pending() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("the read never landed");
}

#[test]
fn a_staged_file_is_read_on_a_worker_and_used_at_send() {
    let root = temp_project("stage");
    fs::write(root.join("brief.md"), "Make it calm.").unwrap();
    let mut state = EditorState::default();
    state.chat.project_files.root = Some(root.to_string_lossy().into_owned());
    state.chat.pending_mentions = vec![file("brief.md")];
    let mut reader = MentionFileReader::new();
    reader.sync(&mut state);
    assert!(reader.is_pending());
    assert!(state
        .chat
        .project_files
        .reading_any(&state.chat.pending_mentions));
    settle(&mut reader, &mut state);
    assert!(!state
        .chat
        .project_files
        .reading_any(&state.chat.pending_mentions));
    // The file changes on disk after staging: the turn uses what was read,
    // without touching the disk again.
    fs::remove_file(root.join("brief.md")).unwrap();
    let block = crate::chat_mention_context::mention_context_for_turn(
        &state,
        &[file("brief.md")],
        Some(&root),
    );
    assert!(block.contains("Make it calm."), "{block}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_read_for_a_folder_no_longer_open_is_dropped() {
    let root = temp_project("moved");
    fs::write(root.join("a.md"), "x").unwrap();
    let mut state = EditorState::default();
    state.chat.project_files.root = Some(root.to_string_lossy().into_owned());
    state.chat.pending_mentions = vec![file("a.md")];
    let mut reader = MentionFileReader::new();
    reader.sync(&mut state);
    state.chat.project_files.root = Some("/elsewhere".into());
    state.chat.project_files.summaries.clear();
    state.chat.pending_mentions.clear();
    settle(&mut reader, &mut state);
    assert!(state.chat.project_files.summaries.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn nothing_staged_does_nothing() {
    let mut state = EditorState::default();
    state.chat.project_files.root = Some("/work".into());
    let mut reader = MentionFileReader::new();
    assert!(!reader.sync(&mut state));
    assert!(!reader.is_pending());
}
