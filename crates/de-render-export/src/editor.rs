//! Editor-state export dispatch.

use std::path::Path;

use de_editor_core::scene_template_catalog::TemplateScene;
use de_editor_core::{EditorState, ExportFormat};

use crate::pdf::{render_deck_pdf_boards_bytes, render_deck_pdf_bytes, render_pdf_bytes};
use crate::{
    render_node_on_page_raster_bytes, render_node_svg_bytes, render_page_raster_bytes,
    render_svg_bytes, ExportError, RasterFormat,
};

/// Scope captured when the shell drains an export action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorExportScope {
    /// Use the format, scale, selection, and scenario currently configured on
    /// the editor state.
    Configured,
    /// Export only these slide boards as a PDF. The ids are a filter; output
    /// order still follows authored deck order.
    DeckBoards(Vec<String>),
}

/// One complete file ready for a native save or share flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportArtifact {
    pub file_name: String,
    pub mime_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Suggested portable name for `scope`.
pub fn default_export_file_name(state: &EditorState, scope: &EditorExportScope) -> String {
    match scope {
        EditorExportScope::Configured => {
            de_editor_core::export_name::default_export_file_name(state)
        }
        EditorExportScope::DeckBoards(_) => format!(
            "{}-slides.pdf",
            de_editor_core::export_name::document_export_stem(state)
        ),
    }
}

/// Render the current editor state into one native-shell-friendly artifact.
pub fn export_editor_state(
    state: &EditorState,
    scope: &EditorExportScope,
) -> Result<ExportArtifact, ExportError> {
    let (mime_type, bytes) = match scope {
        EditorExportScope::DeckBoards(boards) => (
            "application/pdf",
            render_deck_pdf_boards_bytes(state, boards)?,
        ),
        EditorExportScope::Configured => render_configured(state)?,
    };
    Ok(ExportArtifact {
        file_name: default_export_file_name(state, scope),
        mime_type,
        bytes,
    })
}

/// Render and write the configured artifact to `target`. The HTML formats
/// also write their images beside it (see [`write_html_export`]).
pub fn export_editor_state_to_path(
    state: &EditorState,
    scope: &EditorExportScope,
    target: &Path,
) -> Result<(), ExportError> {
    if let (EditorExportScope::Configured, Some(html)) = (
        scope,
        de_codegen::html_target_for(state.editor_ui.export_format),
    ) {
        return write_html_export(state, html, target);
    }
    let artifact = export_editor_state(state, scope)?;
    std::fs::write(target, artifact.bytes).map_err(|e| ExportError::Write(e.to_string()))
}

/// Write the selection (or active page) as flex-layout HTML to `target`,
/// with embedded images extracted into `<file stem>-assets/` next to it and
/// referenced relatively (#170). Only the files the export names are
/// written; an existing asset folder is added to, never cleared.
pub fn write_html_export(
    state: &EditorState,
    target: de_codegen::HtmlTarget,
    path: &Path,
) -> Result<(), ExportError> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("export");
    let asset_dir = format!("{stem}-assets");
    let options = de_codegen::HtmlExportOptions {
        asset_dir: Some(asset_dir),
        ..de_codegen::HtmlExportOptions::default()
    };
    let export = de_codegen::export_editor_html(state, target, &options);
    let write = |p: &Path, bytes: &[u8]| {
        std::fs::write(p, bytes).map_err(|e| ExportError::Write(format!("{}: {e}", p.display())))
    };
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    for asset in &export.assets {
        let file = base.join(&asset.path);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| ExportError::Write(format!("{}: {e}", dir.display())))?;
        }
        write(&file, &asset.bytes)?;
    }
    write(path, export.html.as_bytes())
}

fn render_configured(state: &EditorState) -> Result<(&'static str, Vec<u8>), ExportError> {
    let format = state.editor_ui.export_format;
    // One-file artifact (share sheets, downloads): images stay inline.
    if let Some(target) = de_codegen::html_target_for(format) {
        let options = de_codegen::HtmlExportOptions::default();
        let export = de_codegen::export_editor_html(state, target, &options);
        return Ok(("text/html; charset=utf-8", export.html.into_bytes()));
    }
    if format == ExportFormat::Pdf {
        let bytes = if state.editor_ui.scenario == Some(TemplateScene::Slides) {
            render_deck_pdf_bytes(state)?
        } else {
            let scene = de_pen_loader::editor_state_to_layout_scene(state);
            render_pdf_bytes(&scene)?
        };
        return Ok(("application/pdf", bytes));
    }

    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    let page = scene.active_page().ok_or(ExportError::NoActivePage)?;
    let single_node = (state.selection_count() == 1 && state.selection.anchor.is_real())
        .then(|| state.selection.anchor.as_str());
    let scale = state.editor_ui.export_scale;
    match format {
        ExportFormat::Png => Ok((
            "image/png",
            render_raster(page, single_node, RasterFormat::Png, scale)?,
        )),
        ExportFormat::Jpeg => Ok((
            "image/jpeg",
            render_raster(page, single_node, RasterFormat::Jpeg, scale)?,
        )),
        ExportFormat::Webp => Ok((
            "image/webp",
            render_raster(page, single_node, RasterFormat::Webp, scale)?,
        )),
        ExportFormat::Svg => {
            let bytes = match single_node {
                Some(id) => render_node_svg_bytes(&scene, id)?,
                None => render_svg_bytes(&scene)?,
            };
            Ok(("image/svg+xml", bytes))
        }
        ExportFormat::Pdf => unreachable!("PDF returned before active-page scene construction"),
        ExportFormat::Html | ExportFormat::HtmlTailwind => {
            unreachable!("HTML returned before active-page scene construction")
        }
    }
}

fn render_raster(
    page: &de_editor_ui::layout_scene::ScenePage,
    single_node: Option<&str>,
    format: RasterFormat,
    scale: f32,
) -> Result<Vec<u8>, ExportError> {
    match single_node {
        Some(id) => render_node_on_page_raster_bytes(page, id, format, scale),
        None => render_page_raster_bytes(page, format, scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> EditorState {
        let doc = jian_ops_schema::load_str(
            r##"{"version":"1.0.0","children":[
                {"type":"rectangle","id":"r1","name":"Card","x":0,"y":0,
                 "width":40,"height":30,"fill":[{"type":"solid","color":"#3366ff"}]}
            ]}"##,
        )
        .expect("fixture parses")
        .value;
        let mut state = EditorState::from_document(doc);
        state.editor_ui.file_name_display = Some("mobile.de".into());
        state.editor_ui.export_scale = 1.0;
        state
    }

    fn artifact(format: ExportFormat) -> ExportArtifact {
        let mut state = state();
        state.editor_ui.export_format = format;
        export_editor_state(&state, &EditorExportScope::Configured).expect("export")
    }

    #[test]
    fn configured_formats_emit_their_file_magic_and_names() {
        let png = artifact(ExportFormat::Png);
        assert_eq!(&png.bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(png.file_name, "mobile.png");
        assert_eq!(png.mime_type, "image/png");

        let jpeg = artifact(ExportFormat::Jpeg);
        assert_eq!(&jpeg.bytes[..3], &[0xff, 0xd8, 0xff]);
        assert_eq!(jpeg.file_name, "mobile.jpg");

        let svg = artifact(ExportFormat::Svg);
        assert!(svg.bytes.starts_with(b"<svg "));
        assert_eq!(svg.file_name, "mobile.svg");

        let pdf = artifact(ExportFormat::Pdf);
        assert!(pdf.bytes.starts_with(b"%PDF-"));
        assert_eq!(pdf.file_name, "mobile.pdf");

        let html = artifact(ExportFormat::Html);
        assert!(html.bytes.starts_with(b"<!-- Generated by Design Editor"));
        assert_eq!(html.file_name, "mobile.html");
        assert_eq!(html.mime_type, "text/html; charset=utf-8");
        let tw = artifact(ExportFormat::HtmlTailwind);
        assert!(String::from_utf8(tw.bytes).unwrap().contains("@tailwindcss/browser"));
    }

    #[test]
    fn html_export_writes_images_next_to_the_file() {
        let doc = jian_ops_schema::load_str(
            r#"{"version":"1.0","children":[{"type":"image","id":"i","name":"Logo",
                "width":10,"height":10,"src":"data:image/png;base64,iVBORw0KGgo="}]}"#,
        )
        .expect("fixture")
        .value;
        let mut state = EditorState::from_document(doc);
        state.editor_ui.export_format = ExportFormat::Html;
        let dir = std::env::temp_dir().join(format!("de-html-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("page.html");
        export_editor_state_to_path(&state, &EditorExportScope::Configured, &target)
            .expect("export");
        let html = std::fs::read_to_string(&target).unwrap();
        assert!(html.contains("src=\"page-assets/logo.png\""), "{html}");
        let png = std::fs::read(dir.join("page-assets/logo.png")).unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        std::fs::remove_dir_all(&dir).ok();
    }
}
