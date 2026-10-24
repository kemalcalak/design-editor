//! Editor-state entry for the export dialog's HTML formats: the hosts call
//! this, then write the HTML (and, on desktop, its image files).

use super::{export_html, HtmlExport, HtmlExportOptions, HtmlTarget};
use de_editor_core::walkers::find_node;
use de_editor_core::{EditorState, ExportFormat};
use jian_ops_schema::node::PenNode;

/// The HTML flavour an export-dialog format asks for; `None` for the canvas
/// formats.
pub fn html_target_for(format: ExportFormat) -> Option<HtmlTarget> {
    match format {
        ExportFormat::Html => Some(HtmlTarget::Css),
        ExportFormat::HtmlTailwind => Some(HtmlTarget::Tailwind),
        _ => None,
    }
}

/// The nodes an export covers: the selection (in selection order), else the
/// active page's top-level nodes.
fn export_roots(state: &EditorState) -> Vec<PenNode> {
    let doc = &state.doc;
    let pages: Vec<&[PenNode]> = match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages.iter().map(|p| p.children.as_slice()).collect(),
        _ => vec![doc.children.as_slice()],
    };
    let selected: Vec<PenNode> = state
        .selection
        .set
        .iter()
        .filter_map(|id| pages.iter().find_map(|roots| find_node(roots, id)))
        .cloned()
        .collect();
    if !selected.is_empty() {
        return selected;
    }
    let index = state.ui.active_page_index.min(pages.len().saturating_sub(1));
    pages[index].to_vec()
}

/// Export the selection (or active page) as HTML. Component instances
/// resolve against the whole document.
pub fn export_editor_html(
    state: &EditorState,
    target: HtmlTarget,
    options: &HtmlExportOptions,
) -> HtmlExport {
    let roots = export_roots(state);
    let mut scoped = state.doc.clone();
    scoped.pages = None;
    scoped.children = de_editor_core::ref_resolve::resolve_refs_for_canvas_roots(&roots, &state.doc);
    export_html(&scoped, target, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::NodeId;

    fn state() -> EditorState {
        let doc = jian_ops_schema::load_str(
            r#"{"version":"1.0","pages":[
              {"id":"p1","name":"One","children":[
                {"type":"frame","id":"a","name":"Alpha","width":10,"height":10},
                {"type":"frame","id":"b","name":"Beta","width":20,"height":20}
              ]}
            ]}"#,
        )
        .expect("fixture")
        .value;
        EditorState::from_document(doc)
    }

    #[test]
    fn exports_the_page_or_the_selection() {
        let mut state = state();
        let page = export_editor_html(&state, HtmlTarget::Css, &HtmlExportOptions::default());
        assert!(page.html.contains("class=\"alpha\"") && page.html.contains("class=\"beta\""));
        state.set_single_selection(NodeId::new("b"));
        let one = export_editor_html(&state, HtmlTarget::Css, &HtmlExportOptions::default());
        assert!(!one.html.contains("class=\"alpha\""), "{}", one.html);
        assert!(one.html.contains("class=\"beta\""), "{}", one.html);
    }

    #[test]
    fn only_html_formats_map_to_a_target() {
        assert_eq!(html_target_for(ExportFormat::Html), Some(HtmlTarget::Css));
        assert_eq!(
            html_target_for(ExportFormat::HtmlTailwind),
            Some(HtmlTarget::Tailwind)
        );
        assert_eq!(html_target_for(ExportFormat::Png), None);
    }
}
