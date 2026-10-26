//! Desktop `.de` Save and Open dialog flow (pen.dev `.pen` opens as an import).
//!
//! Owns native pickers, [`run_action`] routing, and error dialogs.

use std::path::PathBuf;

use de_editor_core::EditorState;
use de_host_native::WidgetHostNative;
#[cfg(test)]
use de_host_services::doc_io::active_page_bbox;
use de_host_services::doc_io::{
    load_editor_state_with_report, preserve_app_preferences, save_to_path, set_file_name_display,
    ActionOutcome, DocIoError, ErrorKind,
};

use crate::persistence_error::DocumentOpenError;

/// Extensions for the Save dialog filter: only `.de` documents are written.
pub(crate) const DOCUMENT_EXTENSIONS: &[&str] = &[de_util::document_file::EXTENSION];

/// Extensions for the Open dialog filter: `.de` documents plus pen.dev
/// `.pen` files and PowerPoint `.pptx` decks, which open as an untitled
/// import.
const OPEN_EXTENSIONS: &[&str] = &[
    de_util::document_file::EXTENSION,
    de_util::document_file::PEN_DEV_EXTENSION,
    de_util::document_file::PPTX_EXTENSION,
];

/// Pop a Save dialog (rfd native) and write a standalone document to the
/// chosen path. `Ok(Some(path))` on success, `Ok(None)` on user cancel, `Err`
/// on IO / encode failure.
///
/// Reports `doc_io::DocIoError` — the serializer's own reason — rather than
/// wrapping it: cancellation is already modelled by `Ok(None)`, so the only
/// thing that can fail here is the write, and a one-variant wrapper would add
/// no distinction. (Open is different and does get a wrapper; see
/// [`crate::persistence_error::DocumentOpenError`].)
fn save_as_dialog(state: &EditorState) -> Result<Option<PathBuf>, DocIoError> {
    let Some(path) = pick_save_as_path(state) else {
        return Ok(None);
    };
    save_to_path(state, &path)?;
    Ok(Some(path))
}

/// Pop only the native Save-As picker. Every ordinary desktop and close/reload
/// path uses this before handing serialization to [`crate::save_session`].
/// [`save_as_dialog`] remains only as the standalone synchronous residual.
pub fn pick_save_as_path(state: &EditorState) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(de_i18n::translate(
            state.editor_ui.locale,
            "dialog.pickerSaveTitle",
        ))
        .add_filter(de_editor_ui::PRODUCT_NAME, DOCUMENT_EXTENSIONS)
        .set_file_name(de_util::document_file::UNTITLED_FILE_NAME)
        .save_file()
        .map(de_util::document_file::with_document_suffix)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a Save As outcome says whether the document reached disk"]
pub(crate) enum SaveActionOutcome {
    Saved,
    Cancelled,
    Failed,
}

impl SaveActionOutcome {
    fn into_action_outcome(self) -> ActionOutcome {
        match self {
            Self::Saved => ActionOutcome::Saved,
            Self::Cancelled | Self::Failed => ActionOutcome::Noop,
        }
    }
}

/// Cmd+S — save to `current_path` if known, else fall through to Save As.
pub fn handle_save(
    host: &mut WidgetHostNative,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) -> SaveActionOutcome {
    if let Some(path) = current_path.clone() {
        match save_to_path(host.editor_state(), &path) {
            Err(e) => {
                eprintln!("[save] {e}");
                show_error_dialog(host, ErrorKind::Save, Some(&path), &e.to_string());
                return SaveActionOutcome::Failed;
            }
            Ok(()) => {
                crate::settings_io::touch_recent(host, &path);
                crate::home_previews::queue_after_save(&path);
                set_display_name(host, Some(&path));
                host.editor_state_mut().mark_saved_revision();
                return SaveActionOutcome::Saved;
            }
        }
    }
    handle_save_as(host, current_path, window)
}

fn set_display_name(host: &mut WidgetHostNative, path: Option<&std::path::Path>) {
    set_file_name_display(host.editor_state_mut(), path);
}

fn viewport_size_for_window(window: Option<&winit::window::Window>) -> (f32, f32) {
    window
        .map(|w| {
            let size = w.inner_size();
            let scale = w.scale_factor() as f32;
            (size.width as f32 / scale, size.height as f32 / scale)
        })
        .unwrap_or((super::INITIAL_VIEWPORT_W, super::INITIAL_VIEWPORT_H))
}

pub(crate) fn fit_loaded_document(
    host: &mut WidgetHostNative,
    window: Option<&winit::window::Window>,
) {
    let (vw, vh) = viewport_size_for_window(window);
    host.fit_content_to_viewport(vw, vh);
    host.mark_editor_state_dirty();
}

/// Legacy synchronous Save As through the native dialog.
pub fn handle_save_as(
    host: &mut WidgetHostNative,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) -> SaveActionOutcome {
    handle_save_as_with(host, current_path, window, save_as_dialog, |host, error| {
        eprintln!("[save as] {error}");
        show_error_dialog(host, ErrorKind::Save, None, &error.to_string());
    })
}

fn handle_save_as_with(
    host: &mut WidgetHostNative,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
    save: impl FnOnce(&EditorState) -> Result<Option<PathBuf>, DocIoError>,
    report_error: impl FnOnce(&WidgetHostNative, &DocIoError),
) -> SaveActionOutcome {
    match save(host.editor_state()) {
        Ok(Some(path)) => {
            crate::settings_io::touch_recent(host, &path);
            crate::home_previews::queue_after_save(&path);
            // Mirror handle_save: refresh the in-chrome file name too, not
            // just the OS window title — without this, first Save As writes
            // the file but the file pill keeps showing "Untitled".
            set_display_name(host, Some(&path));
            host.editor_state_mut().mark_saved_revision();
            *current_path = Some(path);
            refresh_title(current_path, window);
            SaveActionOutcome::Saved
        }
        Ok(None) => SaveActionOutcome::Cancelled,
        Err(e) => {
            report_error(host, &e);
            SaveActionOutcome::Failed
        }
    }
}

/// A document that replaced the editor state.
struct OpenedDocument {
    /// The file the user opened; recent files list it.
    source: PathBuf,
    /// Where Cmd+S writes. `None` for a pen.dev import, which stays a
    /// read-only source: the first save asks for a new file.
    bound: Option<PathBuf>,
}

fn finish_open(
    host: &mut WidgetHostNative,
    opened: OpenedDocument,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) {
    fit_loaded_document(host, window);
    crate::settings_io::touch_recent(host, &opened.source);
    *current_path = opened.bound;
    refresh_title(current_path, window);
}

fn load_into_host(
    host: &mut WidgetHostNative,
    path: &std::path::Path,
) -> Result<Option<OpenedDocument>, DocumentOpenError> {
    let locale = host.editor_state().editor_ui.locale;
    let loaded = load_editor_state_with_report(path, locale);
    crate::heap_pressure::schedule_relief("document load parse");
    let loaded = loaded?;
    let mut state = loaded.state;
    preserve_app_preferences(host.editor_state(), &mut state);
    state.editor_ui.home.hide();
    // Someone else's browser nodes wait for the user's Load pages (#20).
    state.hold_browser_pages_if_any();
    let pen_dev_import = loaded.report.pen_dev_import.clone();
    let pptx_import = loaded.report.pptx_import.clone();
    let bound_path = (!loaded.report.is_import()).then(|| path.to_path_buf());
    set_file_name_display(&mut state, bound_path.as_deref());
    state.clear_selection();
    eprintln!(
        "[open] {} active-page top-level nodes",
        state.active_children().len()
    );
    if !host.replace_editor_state(state) {
        return Ok(None);
    }
    host.editor_state_mut().mark_saved_revision();
    host.force_rotate_layer_panel_owner();
    host.mark_editor_state_dirty();
    host.arm_missing_fonts_detection();
    if let Some(report) = pen_dev_import.filter(|report| report.has_losses()) {
        alert_pen_dev_losses(host, path, &report);
    }
    if let Some(warnings) = pptx_import.filter(|warnings| !warnings.is_empty()) {
        alert_import_losses(
            host,
            path,
            ("dialog.pptxImportTitle", "dialog.pptxImportBody"),
            &warnings.join("\n"),
        );
    }
    Ok(Some(OpenedDocument {
        source: path.to_path_buf(),
        bound: bound_path,
    }))
}

/// Tell the user what a pen.dev import could not bring over (shader fills,
/// notes, missing image files, …) instead of dropping it silently.
/// The web host reports the same losses in a toast (`document_io.rs`).
fn alert_pen_dev_losses(
    host: &WidgetHostNative,
    path: &std::path::Path,
    report: &de_pen_loader::pen_dev::PenDevReport,
) {
    alert_import_losses(
        host,
        path,
        ("dialog.penDevImportTitle", "dialog.penDevImportBody"),
        &report.summary(),
    );
}

/// The import-loss dialog: a title over a body naming the file and what
/// did not come over as-is (pen.dev and PowerPoint imports share it).
fn alert_import_losses(
    host: &WidgetHostNative,
    path: &std::path::Path,
    (title_key, body_key): (&'static str, &'static str),
    details: &str,
) {
    let locale = host.editor_state().editor_ui.locale;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let body = de_i18n::translate(locale, body_key)
        .replace("{{name}}", &name)
        .replace("{{details}}", details);
    crate::message_dialog::alert(
        de_i18n::translate(locale, title_key),
        &body,
        rfd::MessageLevel::Warning,
    );
}

/// The picker for a File ▸ Import source that opens like a document.
fn pick_import_source(host: &WidgetHostNative, label: &str, extension: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(de_i18n::translate(
            host.editor_state().editor_ui.locale,
            "dialog.pickerOpenTitle",
        ))
        .add_filter(label, &[extension])
        .pick_file()
}

/// Cmd+O — pop the Open dialog and replace the current document.
pub fn handle_open(
    host: &mut WidgetHostNative,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) -> bool {
    let path = match rfd::FileDialog::new()
        .set_title(de_i18n::translate(
            host.editor_state().editor_ui.locale,
            "dialog.pickerOpenTitle",
        ))
        .add_filter(de_editor_ui::PRODUCT_NAME, OPEN_EXTENSIONS)
        .pick_file()
    {
        Some(p) => p,
        None => return false,
    };
    match load_into_host(host, &path) {
        Ok(Some(opened)) => {
            finish_open(host, opened, current_path, window);
            true
        }
        Ok(None) => false,
        Err(e) => {
            eprintln!("[open] {e}");
            show_error_dialog(host, ErrorKind::Open, Some(&path), &e.to_string());
            false
        }
    }
}

/// Open a drag/drop or file-association path without a picker.
pub fn open_path(
    host: &mut WidgetHostNative,
    path: PathBuf,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) -> bool {
    match load_into_host(host, &path) {
        Ok(Some(opened)) => {
            finish_open(host, opened, current_path, window);
            true
        }
        Ok(None) => false,
        Err(e) => {
            eprintln!("[open] {e}");
            show_error_dialog(host, ErrorKind::Open, Some(&path), &e.to_string());
            false
        }
    }
}

/// Build the layout-resolved scene from the live editor state and
/// dispatch its configured export format to `path`.
///
/// Reports `de_host_services::export::ExportError` — the reason the export
/// core already classified. The only consumer renders it through `Display`
/// (stderr line + the native error dialog's detail body), which is
/// transparent, so the text the user sees is unchanged.
/// The shared "where do I write this export" picker. Both export actions
/// go through it so they cannot drift apart on title, filter or locale.
fn pick_export_path(
    host: &WidgetHostNative,
    filter_label: &str,
    filter_exts: &[&str],
    default_name: &str,
) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title(de_i18n::translate(
            host.editor_state().editor_ui.locale,
            "dialog.pickerExportTitle",
        ))
        .add_filter(filter_label, filter_exts)
        .set_file_name(default_name)
        .save_file()
}

/// Name the export save dialog opens pre-filled with:
/// `<document>-<node>.<ext>`, from the shared derivation the browser
/// download also uses, so the same document and selection produce the
/// same file name on either host.
pub(crate) fn export_dialog_default_name(state: &EditorState) -> String {
    de_editor_core::export_name::default_export_file_name(state)
}

fn export_editor_state_to_path(
    state: &EditorState,
    path: &std::path::Path,
) -> Result<(), de_host_services::export::ExportError> {
    use de_editor_core::editor_ui_state::ExportFormat as Fmt;
    use de_editor_core::scene_template_catalog::TemplateScene;

    let fmt = state.editor_ui.export_format;
    if let Some(target) = de_codegen::html_target_for(fmt) {
        return de_host_services::export::write_html_export(state, target, path);
    }
    let scale = state.editor_ui.export_scale;
    // A single selected node scopes raster and SVG exports to that
    // subtree. PDF remains page-level.
    let single_node = if state.selection_count() == 1 && state.selection.anchor.is_real() {
        Some(state.selection.anchor.as_str())
    } else {
        None
    };
    if fmt == Fmt::Pdf {
        // A deck's pages are its boards, not its canvas pages: the
        // page-level exporter would put every slide plus the gaps
        // between them on one sheet.
        if state.editor_ui.scenario == Some(TemplateScene::Slides) {
            return de_host_services::export_pdf::export_deck_pdf(state, path);
        }
        // PDF is intentionally multi-page; keep the full builder for it.
        let scene = de_pen_loader::editor_state_to_layout_scene(state);
        return de_host_services::export_pdf::export_pdf(&scene, path);
    }

    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    let raster = |rf: de_host_services::export::RasterFormat| match single_node {
        Some(id) => de_host_services::export::export_node_raster(&scene, id, path, rf, scale),
        None => de_host_services::export::export_raster(&scene, path, rf, scale),
    };
    match fmt {
        Fmt::Png => raster(de_host_services::export::RasterFormat::Png),
        Fmt::Jpeg => raster(de_host_services::export::RasterFormat::Jpeg),
        Fmt::Webp => raster(de_host_services::export::RasterFormat::Webp),
        Fmt::Svg => match single_node {
            Some(id) => de_host_services::export::export_node_svg(&scene, id, path),
            None => de_host_services::export::export_svg(&scene, path),
        },
        Fmt::Pdf => unreachable!("PDF returned before active-page scene construction"),
        Fmt::Html | Fmt::HtmlTailwind => {
            unreachable!("HTML returned before active-page scene construction")
        }
    }
}

/// Route a `FileAction` raised by the file-menu dispatcher to the
/// matching dialog flow. The returned [`ActionOutcome`] tells the
/// runner which post-action bookkeeping to run — see its variant
/// docs.
pub fn run_action(
    action: de_editor_core::editor_ui_state::FileAction,
    host: &mut WidgetHostNative,
    current_path: &mut Option<PathBuf>,
    window: Option<&winit::window::Window>,
) -> ActionOutcome {
    use de_editor_core::editor_ui_state::FileAction;
    match action {
        FileAction::New => {
            let mut state = EditorState::starter();
            preserve_app_preferences(host.editor_state(), &mut state);
            state.editor_ui.home.hide();
            // A Home template entry asked for this fresh document; open its
            // panel on the replacement state.
            if let Some(template) = host
                .editor_state_mut()
                .editor_ui
                .home
                .pending_template
                .take()
            {
                state.editor_ui.apply_home_template(template, host.now_ms());
            }
            if !host.replace_editor_state(state) {
                return ActionOutcome::Noop;
            }
            let (vw, vh) = viewport_size_for_window(window);
            host.fit_content_to_viewport(vw, vh);
            host.editor_state_mut().mark_saved_revision();
            // Fresh starter document restarts at revision 0 / page 0 — rotate the
            // LayerPanel cache owner so the next paint rebuilds (same aliasing as
            // the Open path).
            host.force_rotate_layer_panel_owner();
            host.mark_editor_state_dirty();
            *current_path = None;
            refresh_title(current_path, window);
            ActionOutcome::Saved
        }
        FileAction::Home => {
            host.editor_state_mut().editor_ui.entry_surface = de_editor_core::EntrySurface::Home;
            // Opened over the current document, so Escape returns to it.
            host.editor_state_mut().editor_ui.home.show(true);
            host.mark_editor_state_dirty();
            ActionOutcome::Noop
        }
        FileAction::Open => ActionOutcome::saved_or_noop(handle_open(host, current_path, window)),
        FileAction::Save => handle_save(host, current_path, window).into_action_outcome(),
        FileAction::SaveAs => handle_save_as(host, current_path, window).into_action_outcome(),
        FileAction::ExportImage => {
            // main.rs intercepts ExportImage to open the picker; this
            // fallback keeps external callers working.
            host.editor_state_mut().editor_ui.open_export_dialog();
            host.mark_editor_state_dirty();
            ActionOutcome::Noop
        }
        FileAction::ExportImageConfirm => {
            use de_editor_core::editor_ui_state::ExportFormat as Fmt;
            let fmt = host.editor_state().editor_ui.export_format;
            let (filter_label, filter_exts): (&str, &[&str]) = match fmt {
                Fmt::Png => ("PNG", &["png"]),
                Fmt::Jpeg => ("JPEG", &["jpg", "jpeg"]),
                Fmt::Webp => ("WEBP", &["webp"]),
                Fmt::Svg => ("SVG", &["svg"]),
                Fmt::Pdf => ("PDF", &["pdf"]),
                Fmt::Html | Fmt::HtmlTailwind => ("HTML", &["html"]),
            };
            let default_name = export_dialog_default_name(host.editor_state());
            if let Some(path) = pick_export_path(host, filter_label, filter_exts, &default_name) {
                let result = export_editor_state_to_path(host.editor_state(), &path);
                if let Err(e) = result {
                    eprintln!("[export-image] {e}");
                    show_error_dialog(host, ErrorKind::Export, Some(&path), &e.to_string());
                }
            }
            ActionOutcome::Noop
        }
        FileAction::ExportDeckPdfSelection => {
            // The same picker and the same exporter as the PDF branch
            // above, with the board list narrowed to the selection. It
            // does NOT go through `export_editor_state_to_path`: that
            // function's job is "render this document in
            // `export_format`", and the scope here belongs to the action
            // rather than to the format.
            let boards =
                de_editor_core::preview_slideshow::selected_page_boards(host.editor_state());
            if let Some(path) = pick_export_path(host, "PDF", &["pdf"], "design-editor-slides.pdf")
            {
                let result = de_host_services::export_pdf::export_deck_pdf_boards(
                    host.editor_state(),
                    &path,
                    &boards,
                );
                if let Err(e) = result {
                    eprintln!("[export-deck-pdf-selection] {e}");
                    show_error_dialog(host, ErrorKind::Export, Some(&path), &e.to_string());
                }
            }
            ActionOutcome::Noop
        }
        FileAction::ExportAllFrames => {
            host.editor_state_mut()
                .editor_ui
                .image_panel
                .close_popovers();
            crate::persistence_export_batch::handle_export_all_frames(host);
            ActionOutcome::Noop
        }
        FileAction::ExportSlideshowHtml => {
            host.editor_state_mut()
                .editor_ui
                .image_panel
                .close_popovers();
            crate::persistence_export_html::handle_export_slideshow_html(host);
            ActionOutcome::Noop
        }
        FileAction::ExportPptx => {
            host.editor_state_mut()
                .editor_ui
                .image_panel
                .close_popovers();
            crate::persistence_export_pptx::handle_export_pptx(host);
            ActionOutcome::Noop
        }
        FileAction::OpenRecent(i) => {
            let Some(entry) = host.editor_state().editor_ui.recent_files.get(i).cloned() else {
                return ActionOutcome::Noop;
            };
            let path = std::path::PathBuf::from(&entry.path);
            match load_into_host(host, &path) {
                Ok(Some(opened)) => {
                    finish_open(host, opened, current_path, window);
                    ActionOutcome::Saved
                }
                Ok(None) => ActionOutcome::Noop,
                Err(e) => {
                    // File missing / parse failure → tell the user and
                    // drop the stale entry from recents.
                    eprintln!("[open-recent] {e}; pruning {}", entry.path);
                    show_error_dialog(host, ErrorKind::Open, Some(&path), &e.to_string());
                    host.editor_state_mut()
                        .editor_ui
                        .recent_files
                        .retain(|r| r.path != entry.path);
                    host.mark_editor_state_dirty();
                    ActionOutcome::Noop
                }
            }
        }
        FileAction::ClearRecent => {
            host.editor_state_mut().editor_ui.recent_files.clear();
            host.mark_editor_state_dirty();
            ActionOutcome::Noop
        }
        FileAction::ImportHtml => {
            let path = match rfd::FileDialog::new()
                .set_title(de_i18n::translate(
                    host.editor_state().editor_ui.locale,
                    "dialog.pickerOpenTitle",
                ))
                .add_filter("HTML / ZIP", &["html", "htm", "zip"])
                .pick_file()
            {
                Some(p) => p,
                None => return ActionOutcome::Noop,
            };
            // Same worker-thread rationale as the Figma branch: the CSS
            // cascade + resource fetch takes seconds on a real page.
            ActionOutcome::HtmlImportStarted(path)
        }
        FileAction::ImportFigma => {
            let path = match rfd::FileDialog::new()
                .set_title(de_i18n::translate(
                    host.editor_state().editor_ui.locale,
                    "dialog.pickerOpenTitle",
                ))
                .add_filter("Figma", &["fig"])
                .pick_file()
            {
                Some(p) => p,
                None => return ActionOutcome::Noop,
            };
            // Spawn the parse on a worker thread so the UI keeps
            // repainting (a 2–3 MB .fig with hundreds of nodes takes
            // multiple seconds; running it on the main thread freezes
            // the window). The desktop runner picks up the session in
            // the next `RedrawRequested` pump and applies the result
            // when it lands.
            ActionOutcome::FigmaImportStarted(path)
        }
        FileAction::FinishFigmaImport(selection) => ActionOutcome::FigmaImportSelection(selection),
        // Both open like File ▸ Open: the file converts into an untitled
        // document and its loss report is shown there (#72, #246).
        FileAction::ImportPenDev => {
            match pick_import_source(host, "pen.dev", de_util::document_file::PEN_DEV_EXTENSION) {
                Some(path) => {
                    ActionOutcome::saved_or_noop(open_path(host, path, current_path, window))
                }
                None => ActionOutcome::Noop,
            }
        }
        FileAction::ImportPptx => {
            match pick_import_source(host, "PowerPoint", de_util::document_file::PPTX_EXTENSION) {
                Some(path) => {
                    ActionOutcome::saved_or_noop(open_path(host, path, current_path, window))
                }
                None => ActionOutcome::Noop,
            }
        }
        FileAction::ImportImageOrSvg => {
            crate::persistence_image::handle_import_image_or_svg(host);
            ActionOutcome::Noop
        }
        FileAction::PickFillImage => {
            crate::persistence_image::handle_pick_fill_image(host);
            ActionOutcome::Noop
        }
        FileAction::RelinkImage => {
            crate::persistence_image::handle_relink_image(host);
            ActionOutcome::Noop
        }
    }
}

// `import_figma_into_host` (synchronous parse) was retired in favour
// of `figma_import_session::spawn_approved`, which moves the parse to a worker
// thread and pumps the result back through a channel each frame.

pub(crate) fn refresh_title(
    current_path: &Option<PathBuf>,
    window: Option<&winit::window::Window>,
) {
    let Some(window) = window else { return };
    let title = match current_path.as_ref().and_then(|p| p.file_name()) {
        Some(name) => format!(
            "{} — {}",
            name.to_string_lossy(),
            de_editor_ui::PRODUCT_NAME
        ),
        None => de_editor_ui::PRODUCT_NAME.to_string(),
    };
    window.set_title(&title);
}

/// Pop a native error dialog. Used by Open / Save / Export when the
/// underlying IO or parse step fails.
fn show_error_dialog(
    host: &WidgetHostNative,
    kind: ErrorKind,
    path: Option<&std::path::Path>,
    detail: &str,
) {
    let locale = host.editor_state().editor_ui.locale;
    let (title_key, lead_key) = match kind {
        ErrorKind::Open => ("dialog.openErrorTitle", "dialog.openErrorLead"),
        ErrorKind::Save => ("dialog.saveErrorTitle", "dialog.saveErrorLead"),
        ErrorKind::Export => ("dialog.exportErrorTitle", "dialog.exportErrorLead"),
    };
    let mut body = de_i18n::translate(locale, lead_key).to_string();
    if let Some(p) = path {
        body.push_str("\n\n");
        body.push_str(&p.display().to_string());
    }
    body.push_str("\n\n");
    body.push_str(detail);
    crate::message_dialog::alert(
        de_i18n::translate(locale, title_key),
        &body,
        rfd::MessageLevel::Error,
    );
}

/// Public re-export of the native error dialog — used by the
/// background Figma import session (`figma_import_session::pump`) to
/// pop the same OS dialog the synchronous error path uses.
pub fn show_error_dialog_public(
    host: &WidgetHostNative,
    kind: ErrorKind,
    path: Option<&std::path::Path>,
    detail: &str,
) {
    show_error_dialog(host, kind, path, detail)
}

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod tests;
