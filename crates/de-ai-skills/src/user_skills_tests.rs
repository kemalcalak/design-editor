use super::*;

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-user-skills-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn skill(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n")
}

#[test]
fn parse_requires_name_description_and_body() {
    assert!(parse_user_skill(&skill("brand-voice", "House tone", "Write calmly.")).is_ok());
    assert!(matches!(
        parse_user_skill("no front matter"),
        Err(UserSkillError::NoFrontMatter)
    ));
    assert!(matches!(
        parse_user_skill(&skill("Brand Voice", "x", "y")),
        Err(UserSkillError::InvalidName { .. })
    ));
    assert!(matches!(
        parse_user_skill("---\nname: a\n---\nbody\n"),
        Err(UserSkillError::MissingDescription)
    ));
    assert!(matches!(
        parse_user_skill(&skill("a", "d", "")),
        Err(UserSkillError::EmptyBody)
    ));
}

#[test]
fn project_beats_user_and_both_layouts_load() {
    let project = temp("project");
    let user = temp("user");
    fs::create_dir_all(project.join("tone")).unwrap();
    fs::write(
        project.join("tone/SKILL.md"),
        skill("tone", "Project tone", "P"),
    )
    .unwrap();
    fs::write(user.join("tone.md"), skill("tone", "User tone", "U")).unwrap();
    fs::write(user.join("grid.md"), skill("grid", "Grid rules", "G")).unwrap();
    fs::write(user.join("notes.txt"), "ignored").unwrap();
    let set = load_user_skills(
        &[
            (SkillSource::Project, project.clone()),
            (SkillSource::User, user.clone()),
        ],
        &["grid"],
    );
    let names: Vec<&str> = set.skills.iter().map(|s| s.meta.name.as_str()).collect();
    assert_eq!(names, ["grid", "tone"]);
    let tone = set.get("tone").unwrap();
    assert_eq!(tone.source, SkillSource::Project);
    assert_eq!(tone.content, "P");
    assert!(set.get("grid").unwrap().overrides_builtin);
    assert!(!tone.overrides_builtin);
    assert!(set.issues.is_empty());
    fs::remove_dir_all(&project).ok();
    fs::remove_dir_all(&user).ok();
}

#[test]
fn a_broken_skill_is_reported_without_hiding_the_rest() {
    let dir = temp("broken");
    fs::write(dir.join("good.md"), skill("good", "Fine", "Body")).unwrap();
    fs::write(dir.join("bad.md"), "just text").unwrap();
    fs::write(dir.join("big.md"), "x".repeat(MAX_SKILL_BYTES as usize + 1)).unwrap();
    fs::write(dir.join("bin.md"), [0xff, 0xfe, 0x00]).unwrap();
    let set = load_user_skills(&[(SkillSource::User, dir.clone())], &[]);
    assert_eq!(set.skills.len(), 1);
    assert_eq!(set.issues.len(), 3);
    assert!(set
        .issues
        .iter()
        .any(|i| matches!(i.error, UserSkillError::TooLarge { .. })));
    assert!(set
        .issues
        .iter()
        .any(|i| matches!(i.error, UserSkillError::NotUtf8)));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_missing_folder_is_simply_empty() {
    let set = load_user_skills(
        &[(SkillSource::User, PathBuf::from("/definitely/not/here"))],
        &[],
    );
    assert!(set.skills.is_empty() && set.issues.is_empty());
}

#[test]
fn built_in_format_with_phase_and_triggers_also_loads() {
    let text = "---\r\nname: grid\r\ndescription: Grid rules\r\nphase: [planning, generation]\r\ntrigger:\r\n  keywords: [grid]\r\n---\r\nUse 8 px.\r\n";
    let (meta, body) = parse_user_skill(text).unwrap();
    assert_eq!(meta.phase.len(), 2);
    assert_eq!(body, "Use 8 px.");
    let (meta, _) =
        parse_user_skill("---\r\nname: tone\r\ndescription: T\r\n---\r\nCalm\r\n").unwrap();
    assert_eq!(meta.phase, vec![crate::types::Phase::Generation]);
}
