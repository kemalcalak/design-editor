//! Narrowed `EditorState` clones for off-thread request snapshots.
//!
//! Two hot paths hand a whole-editor snapshot to a worker thread:
//!
//!   - `de-host-services::mcp_live`'s `UiRequest::Snapshot` — the live
//!     MCP HTTP server asks the UI thread for a fresh state for *every*
//!     tool call, and an MCP agent issues many per turn.
//!   - `de-host-desktop::chat_session_launch` — the design turn's
//!     `RemoteDocSink` mirror (`de-editor-host-core::design`).
//!
//! Both need an **owned, mutable** `EditorState`: the MCP connection
//! thread re-applies the ack'd command to its local copy so a tool's
//! post-apply read is consistent, and the design worker mutates its
//! mirror. What they *can* skip is the part of the state their consumers
//! never look at ([`narrowed_snapshot`]).
//!
//! The MCP path also avoids copying the document on the UI thread (#352):
//! [`shared_snapshot`] shares unchanged top-level nodes with the previous
//! capture by `Arc`, and the connection thread materializes its owned copy
//! with [`SharedRequestSnapshot::into_state`].
//!
//! ### Field audit (workspace grep over the consumer crates)
//!
//! | Field            | Read downstream? | In the snapshot |
//! | ---------------- | ---------------- | --------------- |
//! | `doc`            | yes (~63 sites)  | cloned          |
//! | `ui`             | yes (~13)        | cloned          |
//! | `selection`      | yes (~9)         | cloned          |
//! | `history`        | yes — `get_history_depth` reads `past/future.len()` | cloned |
//! | `editor_ui`      | yes — `document_save` reads `preserve_authored_geometry` | cloned, except the HTML-import diagnostics rows |
//! | `viewport`       | yes — `get_viewport` | cloned      |
//! | `ui_kits`        | yes — `batch_program` kit lookup | cloned |
//! | `components`     | yes — `tools.rs` resolved_root | cloned |
//! | `clipboard`      | yes — the `paste_clipboard` tool applies `EditorCommand::PasteClipboard` against it | cloned |
//! | `chat`           | **no**           | detached        |
//! | `codegen`        | **no**           | detached        |
//! | `theme_presets`  | **no**           | detached        |
//!
//! `chat` carries every tab's full transcript (design turns park large
//! JSON blocks in assistant messages), `codegen` carries generated
//! source text, and `theme_presets` carries a themes + variables table
//! per saved preset — none of which any snapshot consumer reads, and
//! all of which grow with session length rather than with the document.
//!
//! `editor_ui.html_import_diagnostics` follows the same rule from the
//! opposite direction: it is purely overlay copy (up to `MAX_DIAGNOSTIC_ROWS`
//! rows, each with a code, a locale key, args, and an English sentence), no
//! `EditorCommand` or snapshot consumer reads it, and it is at its largest
//! right after the import a worker is most likely to be asked about. It is
//! taken out with the same take/clone/restore move as `chat`. The scalar
//! `_total` / `_open` / `_expanded` flags stay: they are three words and
//! keep the struct's defaults meaningful.
//!
//! No `EditorCommand` reads `chat` / `codegen` / `theme_presets`
//! either, so re-applying a command against the narrowed copy behaves
//! exactly as it does against a full clone.

use crate::history_snapshot::SharedDoc;
use crate::state::EditorState;

/// Clone `state` for an off-thread consumer, leaving out the
/// session-scoped sub-states audited above.
///
/// Takes `&mut` because the cheap way to skip a field is to move it
/// out, clone, and move it straight back. The take/clone/restore
/// sequence is straight-line on the calling (UI) thread — no early
/// return, no reentrancy — so the live state is whole again before any
/// other code can observe it.
pub fn narrowed_snapshot(state: &mut EditorState) -> EditorState {
    let chat = std::mem::take(&mut state.chat);
    let codegen = std::mem::take(&mut state.codegen);
    let theme_presets = std::mem::take(&mut state.theme_presets);
    let diagnostics = std::mem::take(&mut state.editor_ui.html_import_diagnostics);

    let snapshot = state.clone();

    state.editor_ui.html_import_diagnostics = diagnostics;
    state.chat = chat;
    state.codegen = codegen;
    state.theme_presets = theme_presets;
    snapshot
}

/// A request snapshot whose document is structurally shared with the
/// live editor instead of deep-copied (#352).
///
/// The UI thread builds it with [`shared_snapshot`] — a pointer bump per
/// unchanged top-level node — and the consumer thread turns it into the
/// owned, mutable state it needs with [`Self::into_state`], so the
/// document-sized copy happens off the UI thread.
#[derive(Debug, Clone)]
pub struct SharedRequestSnapshot {
    /// The narrowed state with a children-stripped document.
    rest: EditorState,
    /// The document, shared at top-level-node granularity.
    doc: SharedDoc,
}

impl SharedRequestSnapshot {
    /// Materialize the owned state a consumer can read and mutate. The
    /// result equals what [`narrowed_snapshot`] would have returned at
    /// capture time.
    pub fn into_state(self) -> EditorState {
        let mut state = self.rest;
        state.doc = self.doc.materialize();
        state
    }

    /// The shared document, to pass back as the next capture's anchor.
    pub fn shared_doc(&self) -> &SharedDoc {
        &self.doc
    }
}

/// [`narrowed_snapshot`] with the document shared instead of copied.
///
/// `anchor` is the previous capture's document (see
/// [`SharedRequestSnapshot::shared_doc`]): every top-level node that is
/// deep-equal to the anchor's reuses its `Arc`, so a repeated read call
/// costs one compare pass and no document-sized allocation. Without an
/// anchor the newest undo snapshot stands in, which already shares
/// everything untouched since the last edit; only a document with no
/// history and no previous capture is copied in full.
pub fn shared_snapshot(
    state: &mut EditorState,
    anchor: Option<&SharedDoc>,
) -> SharedRequestSnapshot {
    let anchor = anchor.or_else(|| {
        state
            .history
            .future
            .back()
            .or_else(|| state.history.past.back())
            .map(|snapshot| &snapshot.doc)
    });
    let doc = SharedDoc::capture_parallel(&state.doc, anchor);
    // Clone the rest of the state around a children-stripped stand-in so
    // the node tree is not copied a second time; same straight-line
    // take/clone/restore as `narrowed_snapshot`.
    let live_doc = std::mem::replace(&mut state.doc, doc.skeleton().clone());
    let rest = narrowed_snapshot(state);
    state.doc = live_doc;
    SharedRequestSnapshot { rest, doc }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrowed_snapshot_restores_the_live_state() {
        let mut state = EditorState::new();
        state.chat.title = "long running turn".to_string();
        state.theme_presets.push(crate::theme_presets::ThemePreset {
            id: "p1".into(),
            name: "Preset".into(),
            themes: Default::default(),
            variables: Default::default(),
            created_at: 0,
        });
        state.editor_ui.html_import_diagnostics =
            vec![crate::html_import_diagnostics::HtmlImportDiagnostic::new(
                "layout.float_ignored",
                "htmlImport.warn.layout.float_ignored",
                Vec::new(),
                "CSS float ignored during structured HTML import",
            )];
        state.editor_ui.html_import_diagnostics_total = 1;

        let snapshot = narrowed_snapshot(&mut state);

        // The live state is untouched by the detour.
        assert_eq!(state.chat.title, "long running turn");
        assert_eq!(state.editor_ui.html_import_diagnostics.len(), 1);
        assert_eq!(state.theme_presets.len(), 1);
        // The snapshot keeps everything a consumer reads...
        assert_eq!(snapshot.doc.children.len(), state.doc.children.len());
        assert_eq!(snapshot.revision, state.revision);
        // ...and drops the session-scoped sub-states it does not.
        assert_ne!(snapshot.chat.title, "long running turn");
        assert!(snapshot.theme_presets.is_empty());
        // Overlay-only rows are worker-side dead weight; the scalar total
        // survives so the struct still describes what happened.
        assert!(snapshot.editor_ui.html_import_diagnostics.is_empty());
        assert_eq!(snapshot.editor_ui.html_import_diagnostics_total, 1);
    }
}
