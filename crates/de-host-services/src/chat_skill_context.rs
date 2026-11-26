//! User skills in the chat (#398, aligned with #338): where they are read
//! from, the `/` popup's list, and a picked skill's instructions for the
//! turn.
//!
//! Folders, in precedence order (`de_ai_skills::user_skills`):
//!
//! 1. `<document folder>/.design-editor/skills/` — the project's skills,
//! 2. `<config dir>/skills/` — the user's own (`de_config_store`).
//!
//! The folders are read again whenever the `/` popup opens and at send, so
//! a skill dropped into a folder is picked up without a rebuild or restart.

use std::path::{Path, PathBuf};

use de_ai_skills::user_skills::{load_user_skills, SkillSource, UserSkillSet};
use de_editor_core::chat_mentions::SkillOption;

/// The project skills folder, relative to the document's folder.
pub const PROJECT_SKILLS_DIR: &str = ".design-editor/skills";
/// The user skills folder, inside the app's config directory.
pub const USER_SKILLS_DIR: &str = "skills";
/// Characters of a skill's instructions handed to the model in one turn.
pub const MAX_SKILL_CONTEXT_CHARS: usize = 12_000;

/// The folders skills are read from, project first. The user folder is left
/// out when the config directory cannot be resolved.
pub fn skill_folders(project_root: Option<&Path>) -> Vec<(SkillSource, PathBuf)> {
    let mut folders = Vec::with_capacity(2);
    if let Some(root) = project_root {
        folders.push((SkillSource::Project, root.join(PROJECT_SKILLS_DIR)));
    }
    if let Ok(config) = de_config_store::design_editor_dir() {
        folders.push((SkillSource::User, config.join(USER_SKILLS_DIR)));
    }
    folders
}

/// Load the chat's skills from `folders`, marking those that shadow a
/// built-in skill. Broken files are logged and left out (see
/// `UserSkillSet::issues`); a settings page for them is #338.
pub fn load_chat_skills(folders: &[(SkillSource, PathBuf)]) -> UserSkillSet {
    let builtin: Vec<&str> = de_ai_skills::loader::get_skill_registry()
        .iter()
        .map(|entry| entry.meta.name.as_str())
        .collect();
    let set = load_user_skills(folders, &builtin);
    for issue in &set.issues {
        eprintln!(
            "design-editor: skipped skill {}: {}",
            issue.path.display(),
            issue.error
        );
    }
    set
}

/// The `/` popup's rows for a loaded set.
pub fn skill_options(set: &UserSkillSet) -> Vec<SkillOption> {
    set.skills
        .iter()
        .map(|skill| SkillOption {
            name: skill.meta.name.clone(),
            description: skill.meta.description.clone(),
        })
        .collect()
}

/// List the skills for the popup: the project's and the user's folders.
pub fn scan_chat_skills(project_root: Option<&Path>) -> Vec<SkillOption> {
    skill_options(&load_chat_skills(&skill_folders(project_root)))
}

/// A picked skill's text for the context block: its description and its
/// instructions, cut at [`MAX_SKILL_CONTEXT_CHARS`]. `None` when no folder
/// holds a skill of that name any more.
pub fn skill_instructions(set: &UserSkillSet, name: &str) -> Option<String> {
    let skill = set.get(name)?;
    let body = skill.content.trim();
    let mut text: String = body.chars().take(MAX_SKILL_CONTEXT_CHARS).collect();
    if body.chars().nth(MAX_SKILL_CONTEXT_CHARS).is_some() {
        text.push_str("\n[…skill cut at the context limit]");
    }
    Some(format!("{}\n{text}", skill.meta.description.trim()))
}

#[cfg(test)]
mod tests {
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
    fn options_and_instructions_come_from_the_loaded_set() {
        let dir = temp("set");
        fs::write(
            dir.join("tone.md"),
            "---\nname: tone\ndescription: House tone\n---\nWrite calmly.\n",
        )
        .unwrap();
        let set = load_chat_skills(&[(SkillSource::Project, dir.clone())]);
        assert_eq!(
            skill_options(&set),
            vec![SkillOption {
                name: "tone".into(),
                description: "House tone".into()
            }]
        );
        assert_eq!(
            skill_instructions(&set, "tone").as_deref(),
            Some("House tone\nWrite calmly.")
        );
        assert_eq!(skill_instructions(&set, "gone"), None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn long_instructions_are_cut() {
        let dir = temp("long");
        let body = "x".repeat(MAX_SKILL_CONTEXT_CHARS + 10);
        fs::write(
            dir.join("big.md"),
            format!("---\nname: big\ndescription: Big\n---\n{body}\n"),
        )
        .unwrap();
        let set = load_chat_skills(&[(SkillSource::User, dir.clone())]);
        let text = skill_instructions(&set, "big").unwrap();
        assert!(text.ends_with("[…skill cut at the context limit]"));
        fs::remove_dir_all(&dir).ok();
    }
}
