use super::load_report::is_strict_format_version;
use super::{DocIoError, DocumentLoadReport, LoadedEditorState};
use de_editor_core::EditorState;
use de_pen_loader::{apply_editor_meta, extract_editor_meta_with_report, EditorMeta};

pub fn load_editor_state(
    path: &std::path::Path,
    locale: de_editor_core::Locale,
) -> Result<EditorState, DocIoError> {
    load_editor_state_with_report(path, locale).map(|loaded| loaded.state)
}

/// Load a document from disk. Only `.de` documents and the pen.dev `.pen`
/// and PowerPoint `.pptx` imports open; any other extension is refused
/// before the file is read, and a `.pen` whose content is not pen.dev is
/// refused after the sniff.
pub fn load_editor_state_with_report(
    path: &std::path::Path,
    locale: de_editor_core::Locale,
) -> Result<LoadedEditorState, DocIoError> {
    if de_util::document_file::is_pptx_path(path) {
        return super::pptx::load_pptx(path, locale);
    }
    let pen_dev_path = de_util::document_file::is_pen_dev_path(path);
    if !pen_dev_path && !de_util::document_file::is_document_path(path) {
        return Err(DocIoError::UnsupportedExtension(localized_with_name(
            locale,
            "dialog.loadErrorUnsupportedExtension",
            path,
        )));
    }
    let loaded = read_editor_state_with_report(path, locale)?;
    if pen_dev_path && loaded.report.pen_dev_import.is_none() {
        return Err(DocIoError::NotPenDevDocument(localized_with_name(
            locale,
            "dialog.loadErrorNotPenDev",
            path,
        )));
    }
    Ok(loaded)
}

/// `de_i18n` owns these sentences; `{{name}}` is the file name, not the full
/// path, so the dialog line stays short.
pub(super) fn localized_with_name(
    locale: de_editor_core::Locale,
    key: &'static str,
    path: &std::path::Path,
) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    de_i18n::translate(locale, key).replace("{{name}}", &name)
}

fn read_editor_state_with_report(
    path: &std::path::Path,
    locale: de_editor_core::Locale,
) -> Result<LoadedEditorState, DocIoError> {
    let file = std::fs::File::open(path).map_err(|e| DocIoError::Io(e.to_string()))?;
    let file_len = file
        .metadata()
        .map_err(|e| DocIoError::Io(e.to_string()))?
        .len();
    if file_len == 0 {
        let (state, embedded_meta, report) =
            parse_editor_state_source("", locale, &de_pen_loader::pen_dev::no_assets)?;
        return Ok(finish_loaded_state_with_report(
            state,
            embedded_meta,
            report,
        ));
    }
    // SAFETY: The map is read-only and lives through this synchronous parse.
    // Design Editor atomically replaces its own files rather than truncating the
    // mapped inode in place.
    let bytes = unsafe { memmap2::MmapOptions::new().map(&file) }
        .map_err(|e| DocIoError::Io(e.to_string()))?;
    let src = std::str::from_utf8(bytes.as_ref()).map_err(|error| {
        // `de_i18n` owns this sentence; it is end-user copy, so it is carried
        // through verbatim with its placeholder already substituted.
        DocIoError::InvalidUtf8Document(
            de_i18n::translate(locale, "dialog.loadErrorInvalidUtf8")
                .replace("{{detail}}", &error.to_string()),
        )
    })?;
    let assets = de_pen_loader::pen_dev::folder_assets(path);
    let (state, embedded_meta, report) = parse_editor_state_source(src, locale, &assets)?;
    let mut loaded = finish_loaded_state_with_report(state, embedded_meta, report);
    attach_libraries(&mut loaded, path);
    Ok(loaded)
}

/// Resolve the opened file's linked libraries (#15) and re-read them from
/// disk, so instances show the libraries' current masters. A pen.dev
/// document's `imports` become links; it opens unbound, so they are kept
/// absolute until the first save makes them relative. A library that
/// cannot be read is reported in its status, never as an open error.
fn attach_libraries(loaded: &mut LoadedEditorState, path: &std::path::Path) {
    let state = &mut loaded.state;
    match loaded.report.pen_dev_import.as_ref() {
        Some(report) => {
            let folder = path.parent();
            let links = report
                .imports
                .iter()
                .map(|import| {
                    let file = de_pen_loader::resolve_library_path(folder, &import.path);
                    de_editor_core::LibraryLink::new(
                        de_pen_loader::absolute_library_path(&file),
                        Some(import.alias.clone()),
                    )
                })
                .collect();
            state.editor_ui.libraries.set_links(links);
        }
        None => de_pen_loader::rebase_library_links(state, Some(path)),
    }
    if state.editor_ui.libraries.entries.is_empty() {
        return;
    }
    for problem in de_pen_loader::sync_libraries(state) {
        eprintln!("[open] library not available: {problem}");
    }
}

pub fn load_editor_state_from_source(
    src: &str,
    locale: de_editor_core::Locale,
) -> Result<EditorState, DocIoError> {
    let (state, embedded_meta, _) =
        parse_editor_state_source(src, locale, &de_pen_loader::pen_dev::no_assets)?;
    Ok(finish_loaded_state(state, embedded_meta))
}

/// [`load_editor_state_from_source`] for source text read from `document`
/// by the caller: images the source names in its asset folder are read from
/// beside `document`.
pub fn load_editor_state_from_source_beside(
    src: &str,
    locale: de_editor_core::Locale,
    document: &std::path::Path,
) -> Result<EditorState, DocIoError> {
    let assets = de_pen_loader::pen_dev::folder_assets(document);
    let (state, embedded_meta, _) = parse_editor_state_source(src, locale, &assets)?;
    Ok(finish_loaded_state(state, embedded_meta))
}

fn parse_editor_state_source(
    src: &str,
    locale: de_editor_core::Locale,
    assets: de_pen_loader::pen_dev::AssetReader<'_>,
) -> Result<(EditorState, Option<EditorMeta>, DocumentLoadReport), DocIoError> {
    let embedded = extract_editor_meta_with_report(src);
    let canonical = match de_pen_loader::payload::load_canonical_with_assets(src, assets) {
        Ok(loaded) => loaded,
        Err(error) => {
            if looks_like_legacy_doc_payload(src) {
                return Err(DocIoError::LegacyFormat(
                    de_i18n::translate(locale, "dialog.loadErrorOldVersion").to_string(),
                ));
            }
            // `de_pen_loader` is not owned by this pass — carry its schema
            // rejection verbatim.
            return Err(DocIoError::Schema(error.to_string()));
        }
    };
    let loaded = canonical.loaded;
    for warning in &loaded.warnings {
        eprintln!("[open] schema warning: {warning:?}");
    }
    let malformed_format_version = loaded
        .value
        .format_version
        .as_deref()
        .is_some_and(|version| !is_strict_format_version(version));
    let rewrite_blocked_by_schema_warning = malformed_format_version
        || loaded.warnings.iter().any(|warning| {
            matches!(
                warning,
                jian_ops_schema::LoadWarning::FutureFormatVersion { .. }
            )
        });
    let report = DocumentLoadReport {
        normalized_legacy: canonical.compatibility.normalized_legacy,
        patched_legacy_version: canonical.compatibility.patched_legacy_version,
        inferred_editor_meta: embedded
            .as_ref()
            .is_some_and(|extraction| extraction.inferred_preserve_authored_geometry),
        rewrite_blocked_by_schema_warning,
        pen_dev_import: canonical.compatibility.pen_dev,
        pptx_import: None,
    };
    Ok((
        EditorState::from_document(loaded.value),
        embedded.map(|extraction| extraction.meta),
        report,
    ))
}

fn finish_loaded_state_with_report(
    state: EditorState,
    embedded_meta: Option<EditorMeta>,
    report: DocumentLoadReport,
) -> LoadedEditorState {
    LoadedEditorState {
        state: finish_loaded_state(state, embedded_meta),
        report,
    }
}

pub(super) fn finish_loaded_state(mut state: EditorState, meta: Option<EditorMeta>) -> EditorState {
    if let Some(meta) = meta {
        apply_editor_meta(&mut state, meta);
    } else {
        state.ui.active_page_index = first_page_with_content(&state);
    }
    collapse_top_level_layers(&mut state);
    state
}

/// Start every top-level frame collapsed in the LayerPanel.
///
/// A fully expanded tree is unreadable the moment a document has depth: a
/// six-slide deck opens as ~90 rows and the boards themselves scroll off the
/// panel, so the one thing the list is for — seeing what the document
/// contains — is the first thing lost. Collapsed roots show the six slides
/// and let the user open the one they want.
///
/// `collapsed_layers` is view-only state: not serialized, not in the undo
/// snapshot, and toggling it pushes no history. So this changes what the
/// panel shows on open and nothing about the document.
fn collapse_top_level_layers(state: &mut EditorState) {
    let ids: Vec<de_editor_core::NodeId> = state
        .active_children()
        .iter()
        .filter(|node| {
            // Only containers collapse; a leaf has nothing to hide and would
            // just render a dead disclosure arrow.
            de_editor_core::PenNodeExt::children(*node).is_some_and(|kids| !kids.is_empty())
        })
        .map(|node| de_editor_core::NodeId::new(de_editor_core::PenNodeExt::id_str(node)))
        .collect();
    state.editor_ui.collapsed_layers.extend(ids);
}

fn first_page_with_content(state: &EditorState) -> usize {
    let Some(pages) = state.doc.pages.as_ref() else {
        return 0;
    };
    if pages.first().is_some_and(|page| !page.children.is_empty()) {
        return 0;
    }
    pages
        .iter()
        .position(|page| !page.children.is_empty())
        .unwrap_or(0)
}

pub(super) fn looks_like_legacy_doc_payload(src: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(src) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    let version_is_integer = obj
        .get("version")
        .map(|version| version.is_u64() || version.is_i64())
        .unwrap_or(false);
    version_is_integer
        && obj
            .get("pages")
            .map(serde_json::Value::is_array)
            .unwrap_or(false)
}
