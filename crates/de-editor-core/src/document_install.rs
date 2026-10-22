//! Structural validation of a whole document before it replaces the live one.
//!
//! A server holding one shared editor behind a mutex validates an incoming
//! document off-lock with [`PreparedDocument::prepare`], then hands the
//! validated document back with [`PreparedDocument::into_document`] for the
//! plain replace path.

use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::PenDocument;
use std::collections::HashSet;

/// Structural document failure detected before installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentInstallError {
    /// An explicit page has no stable id.
    EmptyPageId { page_index: usize },
    /// A node has no stable id.
    EmptyNodeId,
    /// A page or node id occurs more than once in the document.
    DuplicateId { id: String },
    /// Explicit pages and legacy root children cannot both carry content.
    ExplicitPagesWithRootChildren,
}

impl std::fmt::Display for DocumentInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPageId { page_index } => {
                write!(f, "page at index {page_index} has an empty id")
            }
            Self::EmptyNodeId => f.write_str("document contains a node with an empty id"),
            Self::DuplicateId { id } => write!(f, "document id is not globally unique: {id}"),
            Self::ExplicitPagesWithRootChildren => {
                f.write_str("a document with explicit pages must have empty root children")
            }
        }
    }
}

impl std::error::Error for DocumentInstallError {}

/// A document that has cleared every structural check an install performs.
///
/// Validation walks the whole node forest, so a server holding one shared
/// editor behind a mutex must not pay for it under the lock. Splitting the
/// fallible half out lets a caller validate off-lock and then replace the
/// document infallibly.
#[derive(Debug, Clone)]
pub struct PreparedDocument {
    doc: PenDocument,
}

impl PreparedDocument {
    /// Run every structural check an install would run.
    pub fn prepare(doc: PenDocument) -> Result<Self, DocumentInstallError> {
        validate_install_document(&doc)?;
        Ok(Self { doc })
    }

    /// The validated document, for a caller that needs to compare before
    /// deciding to install.
    pub fn document(&self) -> &PenDocument {
        &self.doc
    }

    /// Give the validated document back to the caller that replaces the
    /// live document with it.
    pub fn into_document(self) -> PenDocument {
        self.doc
    }
}

fn validate_install_document(doc: &PenDocument) -> Result<(), DocumentInstallError> {
    if doc
        .pages
        .as_ref()
        .is_some_and(|pages| !pages.is_empty() && !doc.children.is_empty())
    {
        return Err(DocumentInstallError::ExplicitPagesWithRootChildren);
    }

    let mut seen = HashSet::new();
    let mut stack = Vec::new();
    if let Some(pages) = doc.pages.as_ref() {
        for (page_index, page) in pages.iter().enumerate() {
            if page.id.is_empty() {
                return Err(DocumentInstallError::EmptyPageId { page_index });
            }
            if !seen.insert(page.id.clone()) {
                return Err(DocumentInstallError::DuplicateId {
                    id: page.id.clone(),
                });
            }
            stack.extend(page.children.iter());
        }
    }
    stack.extend(doc.children.iter());
    while let Some(node) = stack.pop() {
        let id = node.id_str();
        if id.is_empty() {
            return Err(DocumentInstallError::EmptyNodeId);
        }
        if !seen.insert(id.to_string()) {
            return Err(DocumentInstallError::DuplicateId { id: id.to_string() });
        }
        if let Some(children) = node.children() {
            stack.extend(children.iter());
        }
    }
    Ok(())
}
