//! What a preview session is entered with ([`ResetSeed`], kept so a debug
//! reset rebuilds the same session) and the entry-screen choice (#223).

use std::rc::Rc;

use de_editor_core::PenNodeExt;
use jian_core::layout::measure::MeasureBackend;
use jian_ops_schema::PenDocument;

#[derive(Clone)]
pub(crate) struct ResetSeed {
    pub(crate) document: PenDocument,
    pub(crate) canvas_size: (f32, f32),
    pub(crate) active_theme: std::collections::BTreeMap<String, String>,
    pub(crate) active_page_index: usize,
    /// The top-level board Preview opens on
    /// (`de_editor_core::preview_screen_set::preview_entry_root`).
    pub(crate) entry_root_id: Option<String>,
    pub(crate) preserve_authored_geometry: bool,
    pub(crate) presenting: bool,
    pub(crate) measure: Rc<dyn MeasureBackend>,
    pub(crate) host_capabilities: de_preview_contracts::PreviewHostCapabilities,
    pub(crate) host_motion_preference: jian_ops_schema::motion::MotionPreference,
}

/// Make the screen projected from the board `entry_root_id` the entry of
/// a `project_screens` result: the route table's entry and the page jian's
/// loader mounts (`pages[0]`). Every other route stays, so an authored
/// link to `/` still reaches its page. A no-op when the board is not a
/// routed screen, or is one only through responsive variants (their page
/// ids are not the board's).
pub(crate) fn reroot_projection(doc: &mut PenDocument, entry_root_id: &str) {
    let (Some(pages), Some(routes)) = (doc.pages.as_mut(), doc.routes.as_mut()) else {
        return;
    };
    let Some(index) = pages.iter().position(|page| {
        page.children
            .first()
            .is_some_and(|root| root.id_str() == entry_root_id)
    }) else {
        return;
    };
    let page_id = &pages[index].id;
    let Some(path) = routes
        .routes
        .iter()
        .find(|(_, spec)| &spec.page_id == page_id)
        .map(|(path, _)| path.clone())
    else {
        return;
    };
    routes.entry = path;
    let page = pages.remove(index);
    pages.insert(0, page);
}
