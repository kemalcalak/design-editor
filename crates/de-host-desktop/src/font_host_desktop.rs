//! The desktop's [`FontHost`]: what the MCP `load_fonts` tool and the
//! `fontIssues` field of `get_design_quality` run against.
//!
//! Availability is answered the way the canvas resolves a family: faces
//! registered with `jian-skia` (bundled, imported, downloaded) first, then
//! whatever the native font manager can match. Downloads go through the
//! same [`ensure_user_family`] as the missing-fonts modal, so both share the
//! store, the cache and the tracked status.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use de_editor_core::missing_fonts::{
    document_concrete_font_families, missing_font_entries, FontDownloadStatus,
};
use de_editor_core::EditorState;
use de_host_services::font_host::{
    FontHost, FontIssue, FontIssueStatus, FontLoadResult, FontLoadStatus,
};

use crate::fonts::{FontOrigin, FontStore};
use crate::fonts_error::FontDownloadError;
use crate::google_font_download::{ensure_user_family, tracked_status, FontDownloadOk};

/// Called after a download registered new faces, from the download's
/// thread. The GUI wakes its event loop to refresh font snapshots and
/// repaint; headless modes leave it unset.
type FontsChanged = Box<dyn Fn() + Send + Sync>;

static ON_CHANGE: OnceLock<Mutex<Option<FontsChanged>>> = OnceLock::new();

fn on_change() -> &'static Mutex<Option<FontsChanged>> {
    ON_CHANGE.get_or_init(|| Mutex::new(None))
}

/// Install the desktop font host for this process (all modes).
pub(crate) fn install() {
    de_host_services::font_host::install(Box::new(DesktopFontHost));
}

/// Set what runs after a download registered new faces.
pub(crate) fn set_fonts_changed_notifier(notify: impl Fn() + Send + Sync + 'static) {
    if let Ok(mut slot) = on_change().lock() {
        *slot = Some(Box::new(notify));
    }
}

/// Set between a download's notification and the UI's refresh, so a
/// `load_fonts` of many families wakes the loop once, not once per family.
static CHANGE_PENDING: AtomicBool = AtomicBool::new(false);

/// Clear the pending-change flag; the UI calls this as it refreshes.
pub(crate) fn take_fonts_changed() -> bool {
    CHANGE_PENDING.swap(false, Ordering::AcqRel)
}

fn notify_fonts_changed() {
    if CHANGE_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Ok(slot) = on_change().lock() {
        if let Some(notify) = slot.as_ref() {
            notify();
        }
    }
}

/// Families registered with `jian-skia` (bundled, imported, downloaded).
fn registered_families() -> Vec<String> {
    jian_skia::list_families()
        .into_iter()
        .map(|meta| meta.family)
        .chain(jian_skia::list_bundled_families())
        .collect()
}

/// Families `jian-skia` or the native font manager can resolve, among
/// `candidates`, compared case-insensitively.
fn resolvable(candidates: &[String]) -> Vec<String> {
    let mut known = registered_families();
    known.extend(de_host_native::backend::resolvable_system_font_families(
        candidates,
    ));
    candidates
        .iter()
        .filter(|family| known.iter().any(|k| k.eq_ignore_ascii_case(family)))
        .cloned()
        .collect()
}

fn issue_status(status: Option<FontDownloadStatus>) -> (FontIssueStatus, &'static str) {
    match status {
        Some(FontDownloadStatus::Downloading) => (
            FontIssueStatus::Downloading,
            "Downloading from Google Fonts; text uses a fallback face until it lands.",
        ),
        Some(FontDownloadStatus::NotOnGoogle) => (
            FontIssueStatus::NotOnGoogle,
            "Not on Google Fonts; choose an available family.",
        ),
        Some(FontDownloadStatus::Failed) => (
            FontIssueStatus::Failed,
            "The download failed (offline?); call load_fonts to retry or choose another family.",
        ),
        None => (
            FontIssueStatus::Missing,
            "Renders and measures with a fallback face; call load_fonts with this family.",
        ),
    }
}

fn load_result(family: &str, result: &Result<FontDownloadOk, FontDownloadError>) -> FontLoadResult {
    let (status, faces, detail) = match result {
        Ok(FontDownloadOk::Downloaded { faces, .. }) => {
            (FontLoadStatus::Downloaded, Some(*faces), None)
        }
        Ok(FontDownloadOk::Cached { .. }) => (FontLoadStatus::Cached, None, None),
        Err(error) if error.is_not_on_google() => {
            (FontLoadStatus::NotOnGoogle, None, Some(error.to_string()))
        }
        Err(error) => (FontLoadStatus::Failed, None, Some(error.to_string())),
    };
    FontLoadResult {
        family: family.to_string(),
        status,
        faces,
        detail,
    }
}

struct DesktopFontHost;

impl FontHost for DesktopFontHost {
    fn load(&self, families: &[String], wait: Duration) -> Vec<FontLoadResult> {
        let downloaded: Vec<String> = FontStore::user()
            .map(|store| store.families_from(FontOrigin::Google))
            .unwrap_or_default();
        let available = resolvable(families);
        let mut results: Vec<Option<FontLoadResult>> = vec![None; families.len()];
        let (tx, rx) = mpsc::channel();
        for (index, family) in families.iter().enumerate() {
            let is_downloaded = downloaded.iter().any(|d| d.eq_ignore_ascii_case(family));
            if !is_downloaded && available.iter().any(|a| a.eq_ignore_ascii_case(family)) {
                results[index] = Some(FontLoadResult {
                    family: family.clone(),
                    status: FontLoadStatus::Available,
                    faces: None,
                    detail: None,
                });
                continue;
            }
            let tx = tx.clone();
            let family = family.clone();
            // Detached: a download outliving the wait finishes, registers
            // and notifies on its own; the dropped receiver only makes the
            // send a no-op.
            std::thread::spawn(move || {
                let result = ensure_user_family(&family);
                if matches!(result, Ok(FontDownloadOk::Downloaded { .. })) {
                    notify_fonts_changed();
                }
                let _ = tx.send((index, load_result(&family, &result)));
            });
        }
        drop(tx);
        let deadline = Instant::now() + wait;
        while results.iter().any(Option::is_none) {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok((index, result)) => results[index] = Some(result),
                Err(_) => break,
            }
        }
        results
            .into_iter()
            .zip(families)
            .map(|(result, family)| {
                result.unwrap_or_else(|| FontLoadResult {
                    family: family.clone(),
                    status: FontLoadStatus::Downloading,
                    faces: None,
                    detail: None,
                })
            })
            .collect()
    }

    fn font_issues(&self, state: &EditorState) -> Vec<FontIssue> {
        // The GUI's snapshot already carries the enumerated system fonts;
        // reuse them rather than querying the font manager under the global
        // font lock on every quality call. Headless `--mcp` has none.
        let ui = &state.editor_ui;
        let available = if ui.system_fonts_loaded {
            let mut known = registered_families();
            known.extend(ui.system_font_families.iter().cloned());
            known
        } else {
            resolvable(&document_concrete_font_families(state))
        };
        missing_font_entries(state, &|family| {
            available.iter().any(|a| a.eq_ignore_ascii_case(family))
        })
        .into_iter()
        .map(|entry| {
            let (status, hint) = issue_status(tracked_status(&entry.family));
            FontIssue {
                family: entry.family,
                run_count: entry.run_count,
                status,
                hint: hint.to_string(),
            }
        })
        .collect()
    }
}

// Gated off Windows like `fonts.rs`: skia's DirectWrite font manager is not
// safe to drive from cargo's parallel test threads there.
#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    fn state_with_text(family: &str) -> EditorState {
        let doc = serde_json::from_value(serde_json::json!({
            "version": "0.8.0",
            "children": [{
                "type": "text", "id": "t1", "name": "t", "x": 0, "y": 0,
                "width": 10, "height": 10, "content": "hi", "fontFamily": family
            }]
        }))
        .expect("document");
        EditorState::from_document(doc)
    }

    #[test]
    fn a_family_nothing_resolves_is_reported_with_a_load_fonts_hint() {
        let issues = DesktopFontHost.font_issues(&state_with_text("Zz Unheard Of Sans"));
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].family, "Zz Unheard Of Sans");
        assert_eq!(issues[0].status, FontIssueStatus::Missing);
        assert!(issues[0].hint.contains("load_fonts"));
    }

    #[test]
    fn registered_and_generic_families_are_not_issues() {
        crate::bundled_fonts::register();
        assert!(DesktopFontHost
            .font_issues(&state_with_text("Inter, sans-serif"))
            .is_empty());
        assert!(DesktopFontHost
            .font_issues(&state_with_text("sans-serif"))
            .is_empty());
    }

    #[test]
    fn a_registered_family_loads_as_available_without_a_download() {
        crate::bundled_fonts::register();
        let results =
            DesktopFontHost.load(&["Space Grotesk".to_string()], Duration::from_millis(1));
        assert_eq!(results[0].status, FontLoadStatus::Available);
    }
}
