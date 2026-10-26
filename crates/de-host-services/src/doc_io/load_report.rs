//! Compatibility report returned alongside a successfully loaded document.

/// Exact legacy repairs used while opening a document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DocumentLoadReport {
    pub normalized_legacy: bool,
    pub patched_legacy_version: Option<String>,
    pub inferred_editor_meta: bool,
    /// The declared schema is malformed or newer than this build can write.
    pub rewrite_blocked_by_schema_warning: bool,
    /// The file was a pen.dev document converted on open. It stays an
    /// import source: the editor binds no path to it and never rewrites it.
    pub pen_dev_import: Option<de_pen_loader::pen_dev::PenDevReport>,
    /// The file was a PowerPoint deck converted on open (#246): what could
    /// not come over as-is. Like a pen.dev import it binds no path.
    pub pptx_import: Option<Vec<String>>,
}

impl DocumentLoadReport {
    /// Whether a current-shape rewrite is both useful and known to be safe.
    pub fn needs_schema_upgrade(&self) -> bool {
        !self.rewrite_blocked_by_schema_warning
            && !self.is_import()
            && (self.normalized_legacy
                || self.patched_legacy_version.is_some()
                || self.inferred_editor_meta)
    }

    /// Whether the file was converted from another format on open. An
    /// import source is never the document's save path.
    pub fn is_import(&self) -> bool {
        self.pen_dev_import.is_some() || self.pptx_import.is_some()
    }
}

pub(super) fn is_strict_format_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let patch = parts.next();
    parts.next().is_none()
        && [Some(major), Some(minor), patch]
            .into_iter()
            .flatten()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Editor state plus compatibility provenance from the successful load.
pub struct LoadedEditorState {
    pub state: de_editor_core::EditorState,
    pub report: DocumentLoadReport,
}
