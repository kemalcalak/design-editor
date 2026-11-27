use super::*;
use std::fs;

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-chat-skills-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_skill(dir: &Path, name: &str, description: &str, body: &str) {
    fs::write(
        dir.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n"),
    )
    .unwrap();
}

#[test]
fn project_folder_comes_first() {
    let folders = skill_folders(Some(Path::new("/work/site")));
    assert_eq!(
        folders[0],
        (
            SkillSource::Project,
            PathBuf::from("/work/site/.design-editor/skills")
        )
    );
    assert!(folders.iter().skip(1).all(|(s, _)| *s == SkillSource::User));
    assert!(skill_folders(None)
        .iter()
        .all(|(s, _)| *s == SkillSource::User));
}

#[test]
fn options_carry_origin_path_and_shadows() {
    let project = temp("origin-p");
    let user = temp("origin-u");
    write_skill(&project, "tone", "Project tone", "Write calmly.");
    write_skill(&user, "tone", "My tone", "Write loudly.");
    write_skill(&user, "mine", "Mine", "Only mine.");
    let set = load_chat_skills(&[
        (SkillSource::Project, project.clone()),
        (SkillSource::User, user.clone()),
    ]);
    let options = skill_options(&set);
    let tone = options.iter().find(|o| o.name == "tone").unwrap();
    assert_eq!(tone.origin, SkillOrigin::Project);
    assert!(tone.shadows_user);
    assert_eq!(tone.description, "Project tone");
    assert!(tone.path.ends_with("tone.md"));
    assert_eq!(tone.tag_key(), "chat.skill.projectOverUser");
    let mine = options.iter().find(|o| o.name == "mine").unwrap();
    assert_eq!(mine.origin, SkillOrigin::User);
    assert_eq!(mine.tag_key(), "chat.skill.user");
    fs::remove_dir_all(&project).ok();
    fs::remove_dir_all(&user).ok();
}

#[test]
fn instructions_are_read_from_the_file_at_use() {
    let dir = temp("body");
    write_skill(&dir, "tone", "House tone", "Write calmly.");
    let set = load_chat_skills(&[(SkillSource::Project, dir.clone())]);
    let option = skill_options(&set).remove(0);
    assert_eq!(
        skill_instructions(&option).as_deref(),
        Some("House tone\nWrite calmly.")
    );
    // An edit after listing is what the turn gets.
    write_skill(&dir, "tone", "House tone", "Write very calmly.");
    assert_eq!(
        skill_instructions(&option).as_deref(),
        Some("House tone\nWrite very calmly.")
    );
    fs::remove_file(dir.join("tone.md")).unwrap();
    assert_eq!(skill_instructions(&option), None);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn long_instructions_are_cut() {
    let dir = temp("long");
    write_skill(&dir, "big", "Big", &"x".repeat(MAX_SKILL_CONTEXT_CHARS + 10));
    let set = load_chat_skills(&[(SkillSource::User, dir.clone())]);
    let text = skill_instructions(&skill_options(&set)[0]).unwrap();
    assert!(text.ends_with("[…skill cut at the context limit]"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_skill_named_like_a_built_in_becomes_its_override() {
    let dir = temp("override");
    let builtin = de_ai_skills::loader::get_skill_registry()[0]
        .meta
        .name
        .clone();
    write_skill(&dir, &builtin, "Ours", "PROJECT HOUSE RULES");
    write_skill(&dir, "plain", "Plain", "Not an override.");
    let set = load_chat_skills(&[(SkillSource::Project, dir.clone())]);
    let overrides = builtin_overrides(&set);
    assert_eq!(overrides.len(), 1);
    assert_eq!(overrides[0].meta.name, builtin);
    assert_eq!(overrides[0].content.trim(), "PROJECT HOUSE RULES");
    let option = skill_options(&set)
        .into_iter()
        .find(|o| o.name == builtin)
        .unwrap();
    assert_eq!(option.tag_key(), "chat.skill.projectOverBuiltin");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_scanner_lists_off_thread_and_then_answers_from_its_cache() {
    let root = temp("scan");
    let skills = root.join(PROJECT_SKILLS_DIR);
    fs::create_dir_all(&skills).unwrap();
    write_skill(&skills, "tone", "House tone", "Write calmly.");
    let mut scanner = ChatSkillScanner::new();
    assert!(scanner.request(Some(&root)).is_none(), "nothing cached yet");
    assert!(scanner.is_pending());
    let deadline = Instant::now() + Duration::from_secs(10);
    let list = loop {
        if let Some(list) = scanner.poll() {
            break list;
        }
        assert!(Instant::now() < deadline, "the listing never landed");
        thread::sleep(Duration::from_millis(5));
    };
    assert!(!scanner.is_pending());
    assert_eq!(list.root.as_deref(), Some(root.as_path()));
    assert!(list
        .skills
        .iter()
        .any(|s| s.name == "tone" && s.origin == SkillOrigin::Project));
    let cached = scanner.request(Some(&root)).expect("served from the cache");
    assert!(cached.skills.iter().any(|s| s.name == "tone"));
    assert!(!scanner.is_pending(), "a fresh cache lists nothing again");
    fs::remove_dir_all(&root).ok();
}
