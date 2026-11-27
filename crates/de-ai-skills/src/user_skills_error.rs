//! Why a user skill file was not loaded (`user_skills.rs`, #398 / #338).
//! The `Display` text is what a settings page or log line shows next to
//! the file path.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum UserSkillError {
    /// No `---` front matter block, or no `name:` in it.
    NoFrontMatter,
    /// `name` is not a lowercase slug (`a-z`, `0-9`, `-`, `_`, up to 64).
    InvalidName { name: String },
    /// `description` is missing or blank.
    MissingDescription,
    /// Nothing after the front matter.
    EmptyBody,
    /// Over `MAX_SKILL_BYTES`.
    TooLarge { bytes: u64 },
    /// Not UTF-8 text.
    NotUtf8,
    /// A symlink that resolves outside its skills folder.
    OutsideFolder,
    /// The file's front matter no longer names the skill it was listed as.
    NameChanged { listed: String, found: String },
    /// A folder held more skills than `MAX_SKILLS_PER_FOLDER`; the path is
    /// the folder.
    TooManySkills { dropped: usize },
    /// Reading the file failed.
    Io { source: io::Error },
}

impl fmt::Display for UserSkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserSkillError::NoFrontMatter => {
                write!(f, "missing front matter (a --- block with a name)")
            }
            UserSkillError::InvalidName { name } => write!(
                f,
                "name \"{name}\" must be lowercase letters, digits, - or _ (64 at most)"
            ),
            UserSkillError::MissingDescription => write!(f, "description is missing"),
            UserSkillError::EmptyBody => write!(f, "the skill has no instructions"),
            UserSkillError::TooLarge { bytes } => write!(
                f,
                "{bytes} bytes is over the {} byte limit",
                crate::user_skills::MAX_SKILL_BYTES
            ),
            UserSkillError::NotUtf8 => write!(f, "not UTF-8 text"),
            UserSkillError::OutsideFolder => write!(f, "links to a file outside the skills folder"),
            UserSkillError::NameChanged { listed, found } => {
                write!(f, "now names \"{found}\", not \"{listed}\"")
            }
            UserSkillError::TooManySkills { dropped } => write!(
                f,
                "{dropped} skill files past the {} per folder were not loaded",
                crate::user_skills::MAX_SKILLS_PER_FOLDER
            ),
            UserSkillError::Io { source } => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for UserSkillError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UserSkillError::Io { source } => Some(source),
            _ => None,
        }
    }
}
