//! Typed failures for the desktop document Open flow (`persistence.rs`).
//!
//! Style follows `de_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. Every
//! variant wraps an already-typed error from the stage that produced it and
//! `Display` delegates to it, so the `[open]` / `[open-recent]` stderr lines
//! and the native `rfd` Open-error dialog body are unchanged byte for byte.
//!
//! Opening a document has one fallible stage today, parsing it through the
//! canonical loader; the enum keeps the stage attribution so the caller's
//! stale-recent-entry pruning matches on a variant rather than on prose.
//!
//! Save is deliberately NOT modelled here: it is a single stage that reports
//! `de_host_services::doc_io::DocIoError` directly, so wrapping it would add
//! a layer without adding a distinction.

use std::fmt;

use de_host_services::doc_io::DocIoError;

#[derive(Debug)]
pub(crate) enum DocumentOpenError {
    /// The canonical loader refused the file — the "this is not a document we
    /// can read" case, and the only one that justifies pruning a stale recent
    /// entry.
    Load(DocIoError),
}

impl fmt::Display for DocumentOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentOpenError::Load(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DocumentOpenError {}

impl From<DocIoError> for DocumentOpenError {
    fn from(error: DocIoError) -> DocumentOpenError {
        DocumentOpenError::Load(error)
    }
}
