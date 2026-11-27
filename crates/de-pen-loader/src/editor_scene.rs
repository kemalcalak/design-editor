//! Editor-state scene builders.
//!
//! The full builder remains available for export, MCP, and preview consumers.
//! Interactive hosts use the active-page builder so inactive pages do not pay
//! ref/token expansion, layout, payload, or scene-allocation costs.

use std::collections::HashMap;

use de_editor_core::scene_vars::VariableTable;
use jian_scene::layout_scene::{LayoutScene, ScenePage};

use crate::editor_state_var_table;
use crate::payload::{DocPayload, NodePayload};

/// Layer names by scene node id, as the scene builder saw them (instance
/// content under its resolved ids). The SVG export writes them as ids.
pub type SceneLayerNames = HashMap<String, String>;

/// Build a paint-only [`LayoutScene`] from every page in an editor state.
pub fn editor_state_to_layout_scene(state: &de_editor_core::EditorState) -> LayoutScene {
    layout_scene(state, None)
}

/// [`editor_state_to_layout_scene`] plus every node's layer name.
pub fn editor_state_to_layout_scene_with_names(
    state: &de_editor_core::EditorState,
) -> (LayoutScene, SceneLayerNames) {
    let mut names = SceneLayerNames::new();
    let scene = layout_scene(state, Some(&mut names));
    (scene, names)
}

/// [`editor_state_to_active_page_layout_scene`] plus the active page's
/// layer names.
pub fn editor_state_to_active_page_layout_scene_with_names(
    state: &de_editor_core::EditorState,
) -> (LayoutScene, SceneLayerNames) {
    let mut names = SceneLayerNames::new();
    let scene = active_page_layout_scene(state, None, None, Some(&mut names));
    (scene, names)
}

fn collect_names(nodes: &[NodePayload], names: &mut SceneLayerNames) {
    for node in nodes {
        if !node.name.is_empty() {
            names.insert(node.id.clone(), node.name.clone());
        }
        collect_names(&node.children, names);
    }
}

fn layout_scene(
    state: &de_editor_core::EditorState,
    names: Option<&mut SceneLayerNames>,
) -> LayoutScene {
    let mut prepared = std::borrow::Cow::Borrowed(&state.doc);
    if de_editor_core::ref_resolve::document_has_refs(&prepared) {
        prepared = std::borrow::Cow::Owned(de_editor_core::ref_resolve::resolve_refs_for_canvas(
            &prepared,
        ));
    }
    if de_editor_core::variables_resolve::document_has_tokens(&prepared) {
        prepared = std::borrow::Cow::Owned(
            de_editor_core::variables_resolve::resolve_document_for_canvas(
                &prepared,
                &state.ui.variables.active_theme,
            ),
        );
    }
    let payload: DocPayload = if state.editor_ui.preserve_authored_geometry {
        crate::adapter::pen_document_to_payload_preserving_geometry(&prepared).payload
    } else {
        crate::adapter::pen_document_to_payload(&prepared).payload
    };
    let var_table: VariableTable = editor_state_var_table(state);
    if let Some(names) = names {
        for page in &payload.pages {
            collect_names(&page.children, names);
        }
    }

    LayoutScene {
        pages: payload
            .pages
            .iter()
            .map(|page| ScenePage {
                id: page.id.clone(),
                name: page.name.clone(),
                children: page
                    .children
                    .iter()
                    .map(|node| crate::layout_scene::node_payload_to_scene(node, &var_table, 1.0))
                    .collect(),
                image_thumbs: state.image_thumbs.clone().into(),
            })
            .collect(),
        active_page_index: state
            .ui
            .active_page_index
            .min(payload.pages.len().saturating_sub(1)),
    }
}

/// Build the interactive editor scene for only the active page.
///
/// Page metadata stays in document order and `active_page_index` keeps its
/// canonical index, but inactive [`ScenePage`] entries have no render children.
pub fn editor_state_to_active_page_layout_scene(
    state: &de_editor_core::EditorState,
) -> LayoutScene {
    active_page_layout_scene(state, None, None, None)
}

/// [`editor_state_to_active_page_layout_scene`] for page `page_index`
/// instead of the active one (clamped like the active index): a read that
/// names one page lays out that page, not the document (#456). The scene's
/// `active_page_index` is the page laid out.
pub fn editor_state_to_page_layout_scene(
    state: &de_editor_core::EditorState,
    page_index: usize,
) -> LayoutScene {
    active_page_layout_scene(state, Some(page_index), None, None)
}

/// [`editor_state_to_active_page_layout_scene`] with only the page root
/// `root_id` laid out (the active page's other roots are left out; a root
/// id the page does not hold lays out nothing). Page roots are laid out
/// one by one, so the root's boxes match the whole-page scene; reads that
/// look at one board pay for that board instead of the page (#350).
pub fn editor_state_to_active_page_root_layout_scene(
    state: &de_editor_core::EditorState,
    root_id: &str,
) -> LayoutScene {
    active_page_layout_scene(state, None, Some(root_id), None)
}

fn active_page_layout_scene(
    state: &de_editor_core::EditorState,
    page: Option<usize>,
    only_root: Option<&str>,
    names: Option<&mut SceneLayerNames>,
) -> LayoutScene {
    let (mut pages, active_page_index, roots): (
        Vec<ScenePage>,
        usize,
        &[jian_ops_schema::node::PenNode],
    ) = if let Some(doc_pages) = state.doc.pages.as_ref() {
        if doc_pages.is_empty() {
            return LayoutScene::default();
        }
        let active_page_index = page
            .unwrap_or(state.ui.active_page_index)
            .min(doc_pages.len().saturating_sub(1));
        let pages = doc_pages
            .iter()
            .map(|page| ScenePage {
                id: page.id.clone(),
                name: page.name.clone(),
                children: Vec::new(),
                image_thumbs: state.image_thumbs.clone().into(),
            })
            .collect();
        (
            pages,
            active_page_index,
            &doc_pages[active_page_index].children,
        )
    } else {
        let (id, name) = if state.doc.children.is_empty() {
            ("n1".to_string(), "Page 1".to_string())
        } else {
            (
                "page-1".to_string(),
                state.doc.name.as_deref().unwrap_or("Page 1").to_string(),
            )
        };
        (
            vec![ScenePage {
                id,
                name,
                children: Vec::new(),
                image_thumbs: state.image_thumbs.clone().into(),
            }],
            0,
            state.doc.children.as_slice(),
        )
    };

    let roots: &[jian_ops_schema::node::PenNode] = match only_root {
        None => roots,
        Some(id) => roots
            .iter()
            .find(|root| de_editor_core::pen_node_ext::PenNodeExt::id_str(*root) == id)
            .map(std::slice::from_ref)
            .unwrap_or_default(),
    };
    let mut prepared = std::borrow::Cow::Borrowed(roots);
    if de_editor_core::ref_resolve::roots_have_refs(prepared.as_ref()) {
        prepared = std::borrow::Cow::Owned(if state.document_revision() == 0 {
            // A freshly loaded/imported state's registry was built from this
            // exact document. Reusing it avoids indexing every inactive page
            // on each switch. Once any canonical edit has happened the
            // registry may be a stale prototype snapshot, so preserve the old
            // live-document semantics by taking the full lookup path instead.
            de_editor_core::ref_resolve::resolve_refs_for_canvas_roots_with_components(
                prepared.as_ref(),
                &state.components,
                &state.doc,
            )
        } else {
            de_editor_core::ref_resolve::resolve_refs_for_canvas_roots(
                prepared.as_ref(),
                &state.doc,
            )
        });
    }
    if de_editor_core::variables_resolve::roots_have_tokens(prepared.as_ref()) {
        let mut owned = prepared.into_owned();
        de_editor_core::variables_resolve::resolve_roots_for_canvas(
            &mut owned,
            &state.doc,
            &state.ui.variables.active_theme,
        );
        prepared = std::borrow::Cow::Owned(owned);
    }

    let active_meta = &pages[active_page_index];
    let payload = crate::adapter::pen_roots_to_page_payload(
        &active_meta.id,
        &active_meta.name,
        prepared.as_ref(),
        active_page_index,
        state.editor_ui.preserve_authored_geometry,
    );
    let var_table = editor_state_var_table(state);
    if let Some(names) = names {
        collect_names(&payload.children, names);
    }
    pages[active_page_index].children = payload
        .children
        .iter()
        .map(|node| crate::layout_scene::node_payload_to_scene(node, &var_table, 1.0))
        .collect();

    LayoutScene {
        pages,
        active_page_index,
    }
}

/// The laid-out size of node `id` in `scene`, in document px: what a
/// fill-sized script node runs at (#19).
pub fn scene_node_size(scene: &LayoutScene, id: &str) -> Option<(f64, f64)> {
    let node = scene.pages.iter().find_map(|page| page.find(id))?;
    Some((f64::from(node.bounds.size.x), f64::from(node.bounds.size.y)))
}
