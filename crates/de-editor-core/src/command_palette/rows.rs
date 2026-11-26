//! Building the palette's rows (#395): ranked commands, pages, components,
//! variables and recent files for the Commands mode; ranked layer matches
//! and the footer summary for Find layers.

use std::borrow::Cow;

use de_i18n::{translate, translate_with, Locale};

use super::fuzzy::fuzzy_score;
use super::{PaletteCommand, PaletteItem, PaletteRow};
use crate::find_layers::{document_pages, find_layers, FindQuery, MatchField};
use crate::{EditorState, NodeId};

/// A text match's content is cut to this many chars in its row.
const SNIPPET_CHARS: usize = 80;

fn command_score(query: &str, locale: Locale, command: PaletteCommand) -> Option<u32> {
    let spec = command.spec();
    let localized = fuzzy_score(query, translate(locale, spec.label_key));
    let english = fuzzy_score(query, translate(Locale::EnUs, spec.label_key));
    // A keyword hit counts for less than a label hit.
    let keywords = fuzzy_score(query, spec.keywords).map(|score| score / 2);
    localized.max(english).max(keywords)
}

fn command_row(locale: Locale, command: PaletteCommand, score: u32) -> PaletteRow {
    PaletteRow {
        item: PaletteItem::Command(command),
        label: Cow::Borrowed(translate(locale, command.spec().label_key)),
        secondary: Cow::Borrowed(""),
        trailing: Cow::Borrowed(command.chord()),
        score,
    }
}

fn file_name_and_folder(path: &str) -> (String, String) {
    let path = std::path::Path::new(path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let folder = path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    (name, folder)
}

/// Commands mode: every command when nothing is typed, else everything
/// that matches, best first (ties keep the source order: commands, pages,
/// components, variables, recent files).
pub(super) fn command_rows(state: &EditorState, raw: &str) -> Vec<PaletteRow> {
    let locale = state.editor_ui.effective_locale();
    let query = raw.trim().to_lowercase();
    if query.is_empty() {
        return PaletteCommand::ALL
            .iter()
            .map(|&command| command_row(locale, command, 0))
            .collect();
    }
    let mut rows: Vec<PaletteRow> = PaletteCommand::ALL
        .iter()
        .filter_map(|&command| {
            command_score(&query, locale, command).map(|score| command_row(locale, command, score))
        })
        .collect();
    let kind = |key: &'static str| Cow::Borrowed(translate(locale, key));
    if state
        .doc
        .pages
        .as_ref()
        .is_some_and(|pages| !pages.is_empty())
    {
        for (index, name, _) in document_pages(state) {
            if let Some(score) = fuzzy_score(&query, name) {
                rows.push(PaletteRow {
                    item: PaletteItem::Page(index),
                    label: Cow::Owned(name.to_string()),
                    secondary: Cow::Borrowed(""),
                    trailing: kind("palette.kind.page"),
                    score,
                });
            }
        }
    }
    for component in state.components.sorted_options().iter() {
        if let Some(score) = fuzzy_score(&query, &component.name) {
            rows.push(PaletteRow {
                item: PaletteItem::Component(NodeId::new(component.id.as_str())),
                label: Cow::Owned(component.name.clone()),
                secondary: Cow::Borrowed(""),
                trailing: kind("palette.kind.component"),
                score,
            });
        }
    }
    if let Some(variables) = state.doc.variables.as_ref() {
        for name in variables.keys() {
            if let Some(score) = fuzzy_score(&query, name) {
                rows.push(PaletteRow {
                    item: PaletteItem::Variable(name.clone()),
                    label: Cow::Owned(name.clone()),
                    secondary: Cow::Borrowed(""),
                    trailing: kind("palette.kind.variable"),
                    score,
                });
            }
        }
    }
    for (index, recent) in state.editor_ui.recent_files.iter().enumerate() {
        let (name, folder) = file_name_and_folder(&recent.path);
        if let Some(score) = fuzzy_score(&query, &name) {
            rows.push(PaletteRow {
                item: PaletteItem::RecentFile(index),
                label: Cow::Owned(name),
                secondary: Cow::Owned(folder),
                trailing: kind("palette.kind.recent"),
                score,
            });
        }
    }
    rows.sort_by(|a, b| b.score.cmp(&a.score));
    rows
}

fn snippet(text: &str) -> String {
    let mut out: String = text.chars().take(SNIPPET_CHARS).collect();
    if text.chars().nth(SNIPPET_CHARS).is_some() {
        out.push('…');
    }
    out
}

/// Find mode: the ranked matches and the footer summary.
pub(super) fn find_rows(state: &EditorState, query: &FindQuery) -> (Vec<PaletteRow>, String) {
    let locale = state.editor_ui.effective_locale();
    if query.is_empty() {
        return (
            Vec::new(),
            translate(locale, "palette.findHelp").to_string(),
        );
    }
    let results = find_layers(state, query);
    let multi_page = state.page_count() > 1;
    let page_names: Vec<&str> = document_pages(state)
        .into_iter()
        .map(|(_, name, _)| name)
        .collect();
    let rows = results
        .matches
        .into_iter()
        .map(|found| {
            let secondary = match found.field {
                MatchField::Text => found.snippet.as_deref().map(snippet).unwrap_or_default(),
                MatchField::Id => found.node_id.as_str().to_string(),
                MatchField::Name | MatchField::Filter => String::new(),
            };
            let trailing = match page_names.get(found.page_index) {
                Some(name) if multi_page => Cow::Owned(name.to_string()),
                _ => Cow::Borrowed(found.kind),
            };
            PaletteRow {
                label: Cow::Owned(found.name.clone()),
                secondary: Cow::Owned(secondary),
                trailing,
                score: found.score,
                item: PaletteItem::Layer(found),
            }
        })
        .collect();
    (rows, match_summary(locale, results.total, results.pages))
}

/// `12 matches on 3 pages` / `1 match` / `No matches`.
pub(super) fn match_summary(locale: Locale, total: usize, pages: usize) -> String {
    if total == 0 {
        return translate(locale, "palette.noResults").to_string();
    }
    let count = total.to_string();
    let key = match de_i18n::plural_category(locale, total as i64) {
        de_i18n::PluralCategory::One => "palette.matches_one",
        _ => "palette.matches_other",
    };
    let mut summary = translate_with(locale, key, &[("count", count.as_str())]);
    if pages > 1 {
        let pages = pages.to_string();
        summary.push_str(" · ");
        summary.push_str(&translate_with(
            locale,
            "palette.onPages",
            &[("count", pages.as_str())],
        ));
    }
    summary
}

/// `Replaced in 4 layers`.
pub(super) fn replaced_summary(locale: Locale, count: usize) -> String {
    let key = match de_i18n::plural_category(locale, count as i64) {
        de_i18n::PluralCategory::One => "palette.replaced_one",
        _ => "palette.replaced_other",
    };
    let count = count.to_string();
    translate_with(locale, key, &[("count", count.as_str())])
}
