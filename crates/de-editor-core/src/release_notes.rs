//! Compile-time embedded release notes for the "What's new" window (#251).
//!
//! `assets/release_notes.toml` holds one `[[note]]` per (version, locale) in
//! the same small TOML subset as the Prompt Center catalogue, plus a
//! repeatable `item = "…"` key for the bullets. Parsing here keeps it
//! dependency-free and wasm-safe; nothing is fetched at runtime.

use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use de_i18n::Locale;

use crate::catalog_toml::{
    non_empty as toml_non_empty, parse_text as toml_text, required as toml_required,
    set_once as toml_set_once, ValueError,
};
use crate::version_order::is_newer;

const RELEASE_NOTES_TOML: &str = include_str!("../assets/release_notes.toml");

/// The notes of one release in one language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNote {
    /// Dotted release version without a `v` prefix.
    pub version: String,
    pub locale: Locale,
    pub title: String,
    /// One short optional paragraph under the title.
    pub intro: Option<String>,
    /// Bullets in display order; at least one.
    pub items: Vec<String>,
}

/// A checked-in release-notes failure. The line is one-based; `0` names a
/// whole-file rule (a release missing a locale) rather than one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNotesError {
    pub line: usize,
    pub message: String,
}

impl ReleaseNotesError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl From<ValueError> for ReleaseNotesError {
    fn from((line, message): ValueError) -> Self {
        Self { line, message }
    }
}

impl fmt::Display for ReleaseNotesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "release_notes.toml:{}: {}",
            self.line, self.message
        )
    }
}

impl std::error::Error for ReleaseNotesError {}

static RELEASE_NOTES: OnceLock<Vec<ReleaseNote>> = OnceLock::new();

/// Every embedded note, parsed once from the compile-time asset.
pub fn release_notes() -> &'static [ReleaseNote] {
    RELEASE_NOTES
        .get_or_init(|| {
            parse_release_notes(RELEASE_NOTES_TOML)
                .unwrap_or_else(|error| panic!("embedded {error}"))
        })
        .as_slice()
}

/// The notes of `version` in `locale`, else in English.
pub fn note_for(version: &str, locale: Locale) -> Option<&'static ReleaseNote> {
    note_in(release_notes(), version, locale)
}

/// One note per release newer than `after` and no newer than `upto`, in
/// `locale` (else English), newest first. Releases without notes are
/// skipped, so a user who jumped several versions sees each one that has
/// something to say.
pub fn notes_between(after: &str, upto: &str, locale: Locale) -> Vec<&'static ReleaseNote> {
    notes_between_in(release_notes(), after, upto, locale)
}

fn note_in<'a>(notes: &'a [ReleaseNote], version: &str, locale: Locale) -> Option<&'a ReleaseNote> {
    let find = |locale: Locale| {
        notes
            .iter()
            .find(|note| note.version == version && note.locale == locale)
    };
    find(locale).or_else(|| find(Locale::EnUs))
}

fn notes_between_in<'a>(
    notes: &'a [ReleaseNote],
    after: &str,
    upto: &str,
    locale: Locale,
) -> Vec<&'a ReleaseNote> {
    let mut versions: Vec<&str> = Vec::new();
    for note in notes {
        let version = note.version.as_str();
        if is_newer(version, after) && !is_newer(version, upto) && !versions.contains(&version) {
            versions.push(version);
        }
    }
    versions.sort_by(|a, b| {
        if is_newer(a, b) {
            std::cmp::Ordering::Less
        } else if is_newer(b, a) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    versions
        .into_iter()
        .filter_map(|version| note_in(notes, version, locale))
        .collect()
}

/// Whole-file rules: every release carries all 15 UI locales, each with
/// as many bullets as its English note. Runtime lookup falls back to
/// English, so this is enforced by a test rather than at startup.
pub fn check_complete(notes: &[ReleaseNote]) -> Result<(), ReleaseNotesError> {
    let mut versions: Vec<&str> = Vec::new();
    for note in notes {
        if !versions.contains(&note.version.as_str()) {
            versions.push(&note.version);
        }
    }
    for version in versions {
        let english = note_in(notes, version, Locale::EnUs)
            .filter(|note| note.locale == Locale::EnUs)
            .ok_or_else(|| ReleaseNotesError::new(0, format!("{version} has no en-US note")))?;
        for locale in Locale::ALL {
            let note = notes
                .iter()
                .find(|note| note.version == version && note.locale == locale)
                .ok_or_else(|| {
                    ReleaseNotesError::new(0, format!("{version} has no {} note", locale.code()))
                })?;
            if note.items.len() != english.items.len() {
                return Err(ReleaseNotesError::new(
                    0,
                    format!(
                        "{version} {} has {} items, en-US has {}",
                        locale.code(),
                        note.items.len(),
                        english.items.len()
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct NoteBuilder {
    line: usize,
    version: Option<String>,
    locale: Option<Locale>,
    title: Option<String>,
    intro: Option<String>,
    items: Vec<String>,
}

impl NoteBuilder {
    fn finish(
        self,
        seen: &mut HashSet<(String, Locale)>,
    ) -> Result<ReleaseNote, ReleaseNotesError> {
        let line = self.line;
        let version = toml_required(self.version, "version", line)?;
        let locale = toml_required(self.locale, "locale", line)?;
        if !seen.insert((version.clone(), locale)) {
            return Err(ReleaseNotesError::new(
                line,
                format!("duplicate note for {version} {}", locale.code()),
            ));
        }
        if self.items.is_empty() {
            return Err(ReleaseNotesError::new(
                line,
                "a note needs at least one `item`",
            ));
        }
        Ok(ReleaseNote {
            version,
            locale,
            title: toml_non_empty(toml_required(self.title, "title", line)?, "title", line)?,
            intro: self.intro,
            items: self.items,
        })
    }
}

fn parse_version(value: &str, line: usize) -> Result<String, ReleaseNotesError> {
    let version = toml_text(value, line)?;
    let valid = version.split('.').count() == 3
        && version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    if !valid {
        return Err(ReleaseNotesError::new(
            line,
            format!("version `{version}` must be `major.minor.patch`"),
        ));
    }
    Ok(version)
}

fn parse_locale(value: &str, line: usize) -> Result<Locale, ReleaseNotesError> {
    let code = toml_text(value, line)?;
    Locale::ALL
        .into_iter()
        .find(|locale| locale.code() == code)
        .ok_or_else(|| ReleaseNotesError::new(line, format!("unknown locale `{code}`")))
}

/// Parse the strict TOML subset used by the embedded release notes.
pub(crate) fn parse_release_notes(source: &str) -> Result<Vec<ReleaseNote>, ReleaseNotesError> {
    let mut notes = Vec::new();
    let mut current: Option<NoteBuilder> = None;
    let mut seen = HashSet::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line = index + 1;
        let text = raw_line.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        if text == "[[note]]" {
            if let Some(builder) = current.take() {
                notes.push(builder.finish(&mut seen)?);
            }
            current = Some(NoteBuilder {
                line,
                ..NoteBuilder::default()
            });
            continue;
        }
        let builder = current
            .as_mut()
            .ok_or_else(|| ReleaseNotesError::new(line, "field appears before `[[note]]`"))?;
        let (field, value) = text
            .split_once('=')
            .ok_or_else(|| ReleaseNotesError::new(line, "expected `field = value`"))?;
        let (field, value) = (field.trim(), value.trim());
        match field {
            "version" => toml_set_once(
                &mut builder.version,
                parse_version(value, line)?,
                field,
                line,
            )?,
            "locale" => {
                toml_set_once(&mut builder.locale, parse_locale(value, line)?, field, line)?
            }
            "title" => toml_set_once(&mut builder.title, toml_text(value, line)?, field, line)?,
            "intro" => {
                let intro = toml_non_empty(toml_text(value, line)?, field, line)?;
                toml_set_once(&mut builder.intro, intro, field, line)?;
            }
            "item" => builder
                .items
                .push(toml_non_empty(toml_text(value, line)?, field, line)?),
            other => {
                return Err(ReleaseNotesError::new(
                    line,
                    format!("unknown field `{other}`"),
                ));
            }
        }
    }
    if let Some(builder) = current {
        notes.push(builder.finish(&mut seen)?);
    }
    Ok(notes)
}

#[cfg(test)]
#[path = "release_notes_tests.rs"]
mod tests;
