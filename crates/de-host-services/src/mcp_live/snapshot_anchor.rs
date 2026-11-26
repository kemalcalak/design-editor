//! The UI thread's side of a live snapshot (#352, #456): the sharing anchor
//! the next capture compares against, and the pages a narrowed capture
//! keeps.
//!
//! The document of the last snapshot handed to a connection is the anchor:
//! unchanged top-level nodes reuse its `Arc`s, so a read call does not copy
//! the document on the UI thread. It is stamped with when it was handed out
//! and let go after [`SNAPSHOT_ANCHOR_IDLE`], so a document the host
//! replaced (File > Open, revert) is not kept alive by a client that went
//! quiet.
//!
//! A call that reads one page ([`SnapshotPages`], from
//! `de_mcp::snapshot_pages`) captures that page plus the pages its instances
//! may resolve against ([`PageNeeds`], recomputed once per document
//! revision) and leaves the rest out; the anchor keeps the left-out pages it
//! already had, so a later whole-document read still shares them.

use super::*;
use de_editor_core::request_snapshot_pages::{kept_pages, PageNeeds, SnapshotPages};

/// How long the anchor outlives the last snapshot request. Agents call in
/// bursts, so the anchor covers a burst and is dropped once the client goes
/// quiet, freeing nodes no edit still shares.
const SNAPSHOT_ANCHOR_IDLE: Duration = Duration::from_secs(30);

/// UI-thread-only (`pump` takes `&mut self`), so plain fields.
#[derive(Default)]
pub(super) struct SnapshotAnchor {
    doc: Option<(SharedDoc, Instant)>,
    needs: Option<PageNeeds>,
}

impl SnapshotAnchor {
    /// Let go of an anchor nobody asked for in [`SNAPSHOT_ANCHOR_IDLE`].
    pub(super) fn expire_idle(&mut self) {
        if self
            .doc
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > SNAPSHOT_ANCHOR_IDLE)
        {
            self.doc = None;
        }
    }

    /// Nothing in the old document can be shared any more; do not keep it
    /// alive until the next snapshot.
    pub(super) fn clear(&mut self) {
        self.doc = None;
        self.needs = None;
    }

    /// The state a `UiRequest::Snapshot` asked for, or `None` when its
    /// connection already gave up waiting.
    ///
    /// Narrowed: the MCP registry + applier never read `chat` / `codegen` /
    /// `theme_presets`, and those are the sub-states that grow with SESSION
    /// length rather than with the document. See
    /// `de_editor_core::request_snapshot` for the field audit. The
    /// connection materializes its own owned copy off the UI thread
    /// (`ui_requests::request_snapshot`). A request that timed out before
    /// the UI got to it is still skipped, so stale requests piling up behind
    /// a slow frame cost nothing (#350).
    pub(super) fn capture(
        &mut self,
        abandoned: &AtomicBool,
        state: &mut EditorState,
        pages: &SnapshotPages,
    ) -> Option<SharedRequestSnapshot> {
        // A timeout that lands while the capture below runs still costs that
        // one capture; the flag only stops requests that were stale before
        // the UI got to them, which is where the queue piled up.
        if abandoned.load(Ordering::Acquire) {
            return None;
        }
        let keep = self.kept(state, pages);
        let anchor = self.doc.as_ref().map(|(doc, _)| doc);
        let snapshot =
            de_editor_core::request_snapshot::shared_snapshot_pages(state, anchor, keep.as_deref());
        let next = match (&keep, anchor) {
            (Some(keep), Some(anchor)) => anchor.merged_pages(snapshot.shared_doc(), keep),
            _ => snapshot.shared_doc().clone(),
        };
        self.doc = Some((next, Instant::now()));
        Some(snapshot)
    }

    /// The pages to capture for `pages` (`None`: all).
    fn kept(&mut self, state: &EditorState, pages: &SnapshotPages) -> Option<Vec<bool>> {
        if *pages == SnapshotPages::All {
            return None;
        }
        if !self
            .needs
            .as_ref()
            .is_some_and(|needs| needs.is_current(state))
        {
            self.needs = Some(PageNeeds::of(state));
        }
        kept_pages(state, pages, self.needs.as_ref()?)
    }
}
