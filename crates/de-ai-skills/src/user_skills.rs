//! User-installed skills (#398, aligned with #338): `SKILL.md` files the
//! user or the project adds without a rebuild.
//!
//! Two folders are read, in precedence order:
//!
//! 1. the project's `.design-editor/skills/` (next to the open document),
//! 2. the user's skills folder (`<config dir>/skills`),
//!
//! and a project skill shadows a user skill of the same name. A skill named
//! like a built-in skill replaces it in skill resolution
//! ([`crate::loader::set_skill_overrides`]; the host installs them). Each
//! skill is either `<folder>/<name>/SKILL.md` (the Claude-skill layout) or a
//! flat `<folder>/<name>.md`; a symlink that resolves outside its folder is
//! refused. The front matter is the built-in corpus's format (`name`,
//! `description`, optional `phase` / `trigger` / `priority`; no `phase`
//! reads as `generation`), parsed by the same `parse_skill_frontmatter`;
//! `name` and `description` are required here because the chat lists skills
//! by them.
//!
//! Listing reads only each file's front matter ([`load_user_skills`]); the
//! instructions are read when a skill is used ([`read_skill_body`]).
//! Loading never fails as a whole: a broken file becomes a
//! [`UserSkillIssue`] next to the skills that did load, so one bad skill
//! cannot hide the others or break routing.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::frontmatter::{parse_skill_frontmatter, split_frontmatter};
use crate::types::SkillMeta;

pub use crate::user_skills_error::UserSkillError;

/// Largest skill file read, in bytes. A skill is instructions, not data.
pub const MAX_SKILL_BYTES: u64 = 64 * 1024;
/// Bytes read from a skill file to list it (its front matter must end
/// within them).
pub const MAX_FRONT_MATTER_BYTES: u64 = 8 * 1024;
/// Skills read per folder at most.
pub const MAX_SKILLS_PER_FOLDER: usize = 200;

/// Where a skill came from — also its precedence (lower wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkillSource {
    Project,
    User,
}

/// One listed user skill (front matter only).
#[derive(Debug, Clone, PartialEq)]
pub struct UserSkill {
    pub meta: SkillMeta,
    pub source: SkillSource,
    pub path: PathBuf,
    /// A built-in skill of the same name is replaced by this one.
    pub overrides_builtin: bool,
    /// A project skill hiding the user's own skill of the same name.
    pub shadows_user: bool,
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

/// Parse only the front matter of `head` (the start of a skill file).
fn parse_front_matter(head: &str) -> Result<SkillMeta, UserSkillError> {
    let mut lines = head.split_inclusive('\n');
    let mut end = lines.next().ok_or(UserSkillError::NoFrontMatter)?.len();
    let mut closed = false;
    for line in lines {
        end += line.len();
        if line.trim() == "---" {
            closed = true;
            break;
        }
    }
    if !closed {
        return Err(UserSkillError::NoFrontMatter);
    }
    // The body is checked when the skill is used; a stand-in keeps the
    // shared parser's "has a body" rule out of the listing.
    let header = format!("{}\nbody\n", head[..end].trim_end());
    parse_user_skill(&header).map(|(meta, _)| meta)
}

/// The candidate skill files directly in `folder`: `<name>/SKILL.md` and
/// `<name>.md`, each resolved through symlinks and kept only when it stays
/// inside the folder.
fn skill_files(folder: &Path) -> Vec<Result<PathBuf, (PathBuf, UserSkillError)>> {
    let Ok(root) = fs::canonicalize(folder) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
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
    paths.sort();
    paths.truncate(MAX_SKILLS_PER_FOLDER);
    paths
        .into_iter()
        .map(|path| match fs::canonicalize(&path) {
            Ok(real) if real.starts_with(&root) => Ok(path),
            Ok(_) => Err((path, UserSkillError::OutsideFolder)),
            Err(source) => Err((path, UserSkillError::Io { source })),
        })
        .collect()
}

/// Read the front matter of the skill at `path`.
fn read_skill_meta(path: &Path) -> Result<SkillMeta, UserSkillError> {
    let io = |source| UserSkillError::Io { source };
    let size = fs::metadata(path).map_err(io)?.len();
    if size > MAX_SKILL_BYTES {
        return Err(UserSkillError::TooLarge { bytes: size });
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io)?
        .take(MAX_FRONT_MATTER_BYTES)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let head = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        // The read may have cut a character in two at its end.
        Err(err) if err.error_len().is_none() => {
            std::str::from_utf8(&bytes[..err.valid_up_to()]).unwrap_or_default()
        }
        Err(_) => return Err(UserSkillError::NotUtf8),
    };
    parse_front_matter(head)
}

/// Read the instructions of the skill at `path` (the whole file, checked
/// again in full).
pub fn read_skill_body(path: &Path) -> Result<String, UserSkillError> {
    let io = |source| UserSkillError::Io { source };
    let size = fs::metadata(path).map_err(io)?.len();
    if size > MAX_SKILL_BYTES {
        return Err(UserSkillError::TooLarge { bytes: size });
    }
    let bytes = fs::read(path).map_err(io)?;
    let text = String::from_utf8(bytes).map_err(|_| UserSkillError::NotUtf8)?;
    parse_user_skill(&text).map(|(_, body)| body)
}

/// List the skills of every `(source, folder)` pair, in the order given
/// (earlier pairs win a name clash), marking those that replace a built-in
/// skill named in `builtin_names` and project skills that hide a user one.
pub fn load_user_skills(
    folders: &[(SkillSource, PathBuf)],
    builtin_names: &[&str],
) -> UserSkillSet {
    let mut set = UserSkillSet::default();
    for (source, folder) in folders {
        for found in skill_files(folder) {
            let meta = found.and_then(|path| match read_skill_meta(&path) {
                Ok(meta) => Ok((path, meta)),
                Err(error) => Err((path, error)),
            });
            match meta {
                Ok((path, meta)) => {
                    if let Some(winner) = set.skills.iter_mut().find(|s| s.meta.name == meta.name)
                    {
                        // A higher-precedence folder already has it.
                        winner.shadows_user |=
                            winner.source == SkillSource::Project && *source == SkillSource::User;
                        continue;
                    }
                    let overrides_builtin = builtin_names.contains(&meta.name.as_str());
                    set.skills.push(UserSkill {
                        meta,
                        source: *source,
                        path,
                        overrides_builtin,
                        shadows_user: false,
                    });
                }
                Err((path, error)) => set.issues.push(UserSkillIssue {
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
