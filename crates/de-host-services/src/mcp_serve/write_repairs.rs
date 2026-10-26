//! Post-write repairs for bare-MCP writes, scoped to what the write touched.
//!
//! The repairs tidy AI-generated designs (status bars on phone screens, icon
//! paths, empty image slots, a bottom nav appended after a fixed-height
//! wrapper). They used to walk the whole active page, so an unrelated write
//! rewrote any design a person made or imported (#107). Each write now
//! captures a [`WriteBaseline`] before it applies and repairs only the
//! [`WriteScope`] it produced:
//!
//! - status bar / viewport height: roots the write created;
//! - trailing-nav reflow: roots the write created or added nodes into;
//! - icon paths / image slots: nodes the write created or changed.

pub(crate) use de_editor_core::write_scope::{WriteBaseline, WriteScope};
use de_editor_core::EditorState;

/// Tools whose writes must land exactly as authored, so no repair runs after
/// them:
/// - `import_pen_dev` / `import_pptx` create new roots, which [`WriteScope`]
///   would take for AI-generated screens; repairing them rewrites someone
///   else's work.
/// - `execute` (#168) already read back its layout and took its screenshots
///   in the same call; a repair after it would land a document the script
///   never saw.
/// - `undo` / `redo` restore an earlier document; a root they bring back
///   would look new and be repaired, so the step would not round-trip.
pub(crate) fn writes_verbatim(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "import_pen_dev" | "import_pptx" | "execute" | "undo" | "redo"
    )
}

/// The baseline every MCP applier captures right before applying a write
/// of `tool_name`; `None` for a tool whose writes stay verbatim.
pub(crate) fn baseline_for_tool(tool_name: &str, state: &EditorState) -> Option<WriteBaseline> {
    (!writes_verbatim(tool_name)).then(|| WriteBaseline::capture(state))
}

pub(crate) fn normalize_mobile_screens_after_apply(
    state: &mut EditorState,
    in_scope: impl Fn(&str) -> bool,
) -> de_chat_agent::MobileNormalizeReport {
    let report = de_chat_agent::normalize_mobile_screens_for_roots(state, in_scope);
    let mut changes = Vec::new();
    if report.status_bars_inserted > 0 {
        changes.push(format!(
            "status_bars_inserted={}",
            report.status_bars_inserted
        ));
    }
    if report.status_bars_replaced > 0 {
        changes.push(format!(
            "status_bars_replaced={}",
            report.status_bars_replaced
        ));
    }
    if report.duplicate_status_bars_removed > 0 {
        changes.push(format!(
            "duplicate_status_bars_removed={}",
            report.duplicate_status_bars_removed
        ));
    }
    if report.viewport_heights_fixed > 0 {
        changes.push(format!(
            "viewport_heights_fixed={}",
            report.viewport_heights_fixed
        ));
    }
    if !changes.is_empty() {
        eprintln!(
            "design-editor mcp: mobile normalization: {}",
            changes.join(", ")
        );
    }
    report
}

pub(crate) fn normalize_icon_paths_after_apply(
    state: &mut EditorState,
    in_scope: impl Fn(&str) -> bool,
) -> de_editor_core::icon_path_normalize::IconPathNormalizeReport {
    let report = de_editor_core::icon_path_normalize::normalize_icon_paths_for_ids(
        state,
        de_editor_ui::widgets::icons::lucide_name_for_path_d,
        in_scope,
    );
    if report != Default::default() {
        eprintln!(
            "design-editor mcp: icon path normalization: converted_to_icon_font={}, refit_uniform={}",
            report.converted_to_icon_font, report.refit_uniform
        );
    }
    report
}

/// Run the cheap, idempotent repair passes shared by every bare-MCP document
/// write over the part of the page `scope` names, after the command itself
/// has applied and before it saves. Returns whether the trailing-nav reflow
/// or the image-slot pass changed the document: those edit roots that were
/// already laid out, while status bars and heights land only on roots this
/// write created, which the caller already treats as a layout change.
pub(crate) fn run_write_repairs_after_apply(state: &mut EditorState, scope: &WriteScope) -> bool {
    if scope.touched.is_empty() {
        return false;
    }
    let is_touched = |id: &str| scope.touched.contains(id);
    normalize_mobile_screens_after_apply(state, |id| scope.is_new_root(id));
    normalize_icon_paths_after_apply(state, is_touched);
    let mobile_nav_reflowed =
        de_orchestrator::repair_mobile_trailing_nav_reflow_for_roots(state, |id| {
            scope.is_new_or_extended_root(id)
        });
    let image_slots_materialized =
        de_orchestrator::cleanup_image_slots::materialize_empty_image_fill_slots_for_ids(
            state, is_touched,
        );
    mobile_nav_reflowed || image_slots_materialized
}

#[cfg(test)]
#[path = "write_repairs_tests.rs"]
mod tests;
