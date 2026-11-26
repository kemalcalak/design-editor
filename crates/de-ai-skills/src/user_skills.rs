//! User-installed skills (#398, aligned with #338): `SKILL.md` files the
//! user or the project adds without a rebuild.
//!
//! Two folders are read, in precedence order:
//!
//! 1. the project's `.design-editor/skills/` (next to the open document),
//! 2. the user's skills folder (`<config dir>/skills`),
//!
//! and a skill there shadows a built-in skill of the same name. Each skill
//! is either `<folder>/<name>/SKILL.md` (the Claude-skill layout) or a flat
//! `<folder>/<name>.md`. The front matter is the built-in corpus's format
//! (`name`, `description`, optional `phase` / `trigger` / `priority`; no
//! `phase` reads as `generation`), parsed
//! by the same `parse_skill_frontmatter`; `name` and `description` are
//! required here because the chat lists skills by them.
//!
//! Loading never fails as a whole: a broken file becomes a
//! [`UserSkillIssue`] next to the skills that did load, so one bad skill
//! cannot hide the others or break routing.

use std::fs;
use std::path::{Path, PathBuf};

use crate::frontmatter::{parse_skill_frontmatter, split_frontmatter};
use crate::types::SkillMeta;

pub use crate::user_skills_error::UserSkillError;

/// Largest skill file read, in bytes. A skill is instructions, not data.
pub const MAX_SKILL_BYTES: u64 = 64 * 1024;
/// Skills read per folder at most.
pub const MAX_SKILLS_PER_FOLDER: usize = 200;

/// Where a skill came from — also its precedence (lower wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillSource {
    Project,
    User,
}

/// One loaded user skill.
#[derive(Debug, Clone, PartialEq)]
pub struct UserSkill {
    pub meta: SkillMeta,
    /// The markdown body (front matter stripped).
    pub content: String,
    pub source: SkillSource,
    pub path: PathBuf,
    /// True when a built-in skill of the same name is shadowed.
    pub overrides_builtin: bool,
}

/// A skill file that could not be loaded, and why.
#[derive(Debug)]
pub struct UserSkillIssue {
    pub path: PathBuf,
    pub source: SkillSource,
    pub error: UserSkillError,
}

/// What a scan found: usable skills (deduplicated by precedence, sorted by
/// name) and the files that failed validation.
#[derive(Debug, Default)]
pub struct UserSkillSet {
    pub skills: Vec<UserSkill>,
    pub issues: Vec<UserSkillIssue>,
}

impl UserSkillSet {
    /// The skill called `name`, if any.
    pub fn get(&self, name: &str) -> Option<&UserSkill> {
        self.skills.iter().find(|skill| skill.meta.name == name)
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Parse and validate one skill file's text.
///
/// A Claude-style skill carries only `name` and `description`; the built-in
/// format also needs `phase`. A file without a `phase` line is read as a
/// generation-phase skill, so both shapes load.
pub fn parse_user_skill(text: &str) -> Result<(SkillMeta, String), UserSkillError> {
    let (front, _) = split_frontmatter(text).ok_or(UserSkillError::NoFrontMatter)?;
    let declares_phase = front
        .lines()
        .any(|line| line.trim_start().starts_with("phase:") && !line.starts_with(' '));
    let parsed = if declares_phase {
        parse_skill_frontmatter(text)
    } else {
        // Insert after the opening fence line, whatever its line ending.
        let fence_end = text.find('\n').map_or(text.len(), |i| i + 1);
        let with_phase = format!(
            "{}phase: [generation]\n{}",
            &text[..fence_end],
            &text[fence_end..]
        );
        parse_skill_frontmatter(&with_phase)
    };
    let (meta, content) = parsed.ok_or(UserSkillError::NoFrontMatter)?;
    if !valid_name(&meta.name) {
        return Err(UserSkillError::InvalidName {
            name: meta.name.clone(),
        });
    }
    if meta.description.trim().is_empty() {
        return Err(UserSkillError::MissingDescription);
    }
    if content.trim().is_empty() {
        return Err(UserSkillError::EmptyBody);
    }
    Ok((meta, content))
}

/// The skill files directly in `folder`: `<name>/SKILL.md` and `<name>.md`.
fn skill_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name();
            if name.to_str().is_none_or(|n| n.starts_with('.')) {
                return None;
            }
            if path.is_dir() {
                let skill = path.join("SKILL.md");
                skill.is_file().then_some(skill)
            } else if path.extension().is_some_and(|e| e == "md") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files.truncate(MAX_SKILLS_PER_FOLDER);
    files
}

fn read_skill(path: &Path) -> Result<(SkillMeta, String), UserSkillError> {
    let io = |source| UserSkillError::Io { source };
    let size = fs::metadata(path).map_err(io)?.len();
    if size > MAX_SKILL_BYTES {
        return Err(UserSkillError::TooLarge { bytes: size });
    }
    let bytes = fs::read(path).map_err(io)?;
    let text = String::from_utf8(bytes).map_err(|_| UserSkillError::NotUtf8)?;
    parse_user_skill(&text)
}

/// Load the skills of every `(source, folder)` pair, in the order given
/// (earlier pairs win a name clash), marking those that shadow a built-in
/// skill named in `builtin_names`.
pub fn load_user_skills(
    folders: &[(SkillSource, PathBuf)],
    builtin_names: &[&str],
) -> UserSkillSet {
    let mut set = UserSkillSet::default();
    for (source, folder) in folders {
        for path in skill_files(folder) {
            match read_skill(&path) {
                Ok((meta, content)) => {
                    if set.get(&meta.name).is_some() {
                        // A higher-precedence folder already has it.
                        continue;
                    }
                    let overrides_builtin = builtin_names.contains(&meta.name.as_str());
                    set.skills.push(UserSkill {
                        meta,
                        content,
                        source: *source,
                        path,
                        overrides_builtin,
                    });
                }
                Err(error) => set.issues.push(UserSkillIssue {
                    path,
                    source: *source,
                    error,
                }),
            }
        }
    }
    set.skills.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
    set
}

#[cfg(test)]
#[path = "user_skills_tests.rs"]
mod tests;
