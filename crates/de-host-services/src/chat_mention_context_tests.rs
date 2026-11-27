use super::*;
use de_editor_core::chat_mentions::ChatMention;

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-mention-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn file(kind_key: &str) -> ChatMention {
    ChatMention {
        kind: MentionKind::File,
        key: kind_key.into(),
        label: kind_key.into(),
    }
}

#[test]
fn text_files_are_quoted_and_cut() {
    let root = temp_project("text");
    fs::write(root.join("notes.md"), "line one\nline two").unwrap();
    let long: String = (0..200).map(|i| format!("row {i}\n")).collect();
    fs::write(root.join("long.txt"), &long).unwrap();
    assert_eq!(
        summarize_project_file(&root, "notes.md").unwrap(),
        "text file, 17 bytes:\n```\nline one\nline two\n```"
    );
    let cut = summarize_project_file(&root, "long.txt").unwrap();
    assert!(cut.contains(", excerpt:"));
    assert!(cut.contains("row 79") && !cut.contains("row 80"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn binary_files_are_described_not_quoted() {
    let root = temp_project("bin");
    fs::write(root.join("shot.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
    assert_eq!(
        summarize_project_file(&root, "shot.png").unwrap(),
        "binary file (image/png), 7 bytes — attach the image to send its pixels"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn paths_outside_the_project_are_refused() {
    let root = temp_project("escape");
    for rel in ["../secret.txt", "/etc/hosts", "docs/../../x"] {
        assert!(matches!(
            summarize_project_file(&root, rel),
            Err(MentionFileError::OutsideProject { .. })
        ));
    }
    assert!(matches!(
        summarize_project_file(&root, "missing.txt"),
        Err(MentionFileError::Io { .. })
    ));
    fs::remove_dir_all(&root).ok();
}

#[cfg(unix)]
#[test]
fn symlinks_out_of_the_project_are_refused() {
    let root = temp_project("link");
    let outside = temp_project("outside");
    fs::write(outside.join("secret.txt"), "s").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();
    assert!(matches!(
        summarize_project_file(&root, "link.txt"),
        Err(MentionFileError::OutsideProject { .. })
    ));
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&outside).ok();
}

#[test]
fn turn_context_resolves_documents_and_files_in_order() {
    let root = temp_project("turn");
    fs::write(root.join("brief.md"), "Make it calm.").unwrap();
    let state = EditorState::default();
    assert_eq!(mention_context_for_turn(&state, &[], Some(&root)), "");
    let block = mention_context_for_turn(
        &state,
        &[
            file("brief.md"),
            ChatMention {
                kind: MentionKind::Node,
                key: "gone".into(),
                label: "Gone".into(),
            },
        ],
        Some(&root),
    );
    assert!(block.contains("- @brief.md (file): text file, 13 bytes:\n```\nMake it calm.\n```"));
    assert!(block.contains("- @Gone (layer): no longer exists in the document"));
    let unsaved = mention_context_for_turn(&state, &[file("brief.md")], None);
    assert!(unsaved.contains("unreadable — the document is not saved"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_picked_skill_brings_its_instructions() {
    use de_ai_skills::user_skills::SkillSource;
    let dir = temp_project("skill");
    fs::write(
        dir.join("tone.md"),
        "---\nname: tone\ndescription: House tone\n---\nWrite calmly.\n",
    )
    .unwrap();
    let set = crate::chat_skill_context::load_chat_skills(&[(SkillSource::Project, dir.clone())]);
    let mut state = EditorState::default();
    state.chat.skills.skills = crate::chat_skill_context::skill_options(&set).into();
    let skill = |name: &str| ChatMention {
        kind: MentionKind::Skill,
        key: name.into(),
        label: name.into(),
    };
    let summaries = resolve_mentions(&state, &[skill("tone"), skill("gone")], None);
    assert_eq!(
        summaries[0].text.as_deref(),
        Some("House tone\nWrite calmly.")
    );
    assert_eq!(summaries[1].text, None);
    let block = de_editor_core::chat_mentions::context_block(&summaries);
    assert!(block.contains("- /tone (skill): House tone\nWrite calmly."));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn taking_the_turn_mentions_uses_and_drops_the_prefetched_summaries() {
    let mut state = EditorState::default();
    state.chat.project_files.root = Some("/no/such/folder".into());
    state.chat.pending_mentions = vec![file("brief.md")];
    state
        .chat
        .project_files
        .summaries
        .insert("brief.md".into(), Some("text file, 4 bytes: calm".into()));
    let block = take_turn_mentions(&mut state, 0);
    assert!(
        block.contains("- @brief.md (file): text file, 4 bytes: calm"),
        "{block}"
    );
    assert!(state.chat.pending_mentions.is_empty());
    assert!(state.chat.project_files.summaries.is_empty());
}

#[test]
fn the_picked_skills_share_one_budget_and_a_cut_is_announced() {
    let skill = |name: &str| ChatMention {
        kind: MentionKind::Skill,
        key: name.into(),
        label: name.into(),
    };
    let mut state = EditorState::default();
    state.chat.pending_mentions = vec![skill("a"), skill("b"), skill("c")];
    let big = "x".repeat(MAX_TURN_SKILL_CHARS - 10);
    let instructions = &mut state.chat.skills.instructions;
    instructions.insert("a".into(), Some(Some(big.clone())));
    instructions.insert("b".into(), Some(Some("y".repeat(100))));
    instructions.insert("c".into(), Some(Some("z".repeat(100))));
    let block = take_turn_mentions(&mut state, 7);
    assert!(block.contains(&big));
    assert!(block.contains("[…cut at the turn's skill limit]"));
    assert!(block.contains("[left out: the turn's skill limit was reached]"));
    let toast = state.editor_ui.editor_toast.clone().unwrap();
    assert_eq!(toast.i18n_key, SKILLS_CUT_TOAST);
    assert!(state.chat.skills.instructions.is_empty());
}
