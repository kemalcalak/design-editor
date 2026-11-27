//! Structurally-shared snapshot representation for undo / redo.
//!
//! Every undoable gesture used to deep-clone the entire `PenDocument`
//! (node tree + all component prototypes) into an [`crate::history::EditorSnapshot`],
//! and `HISTORY_CAP = 100` kept up to 100 such clones alive at once —
//! peak memory ~100× the document.
//!
//! This module replaces the snapshot's owned `PenDocument` /
//! `ComponentLibrary` with a **share-at-top-level-node granularity**
//! representation:
//!
//!   - [`SharedDoc`] keeps a children-stripped `PenDocument` *skeleton*
//!     (all scalar / collection fields — version, name, themes,
//!     variables, page metadata, app/state/lifecycle/… — cloned as
//!     today) plus, for the single-page root AND every page, the
//!     top-level nodes as `Vec<Arc<PenNode>>`.
//!   - [`SharedComponents`] keeps the component prototypes as
//!     `Vec<Arc<Component>>` (id-keyed sharing).
//!
//! On [`SharedDoc::capture`] each top-level entry is compared against
//! the *anchor* snapshot (the adjacent history state); an unchanged
//! subtree reuses the anchor's `Arc` (a pointer bump, no deep clone),
//! so 100 snapshots retain one copy of every subtree that never
//! changed plus the deltas. Matching is an index fast path (same slot,
//! same id, deep-equal) with an id-keyed fallback map for equal-content
//! duplicates — equality always gates reuse, so a reused `Arc` is
//! byte-identical to what a full clone would have produced.
//!
//! Snapshot-stored `Arc`s are treated **copy-on-write**: the one
//! in-place mutator of stored snapshots
//! ([`crate::instance_override`]'s `repair_scope_snapshots`) routes
//! through [`SharedDoc::repair_swap`], which `Arc::make_mut`s only the
//! affected top-level entry — cloning it away from any sibling snapshot
//! that shares it, so no other history state is contaminated.

use crate::component_backing::DocumentComponentBacking;
use crate::components::{Component, ComponentLibrary};
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::page::PenPage;
use jian_ops_schema::variable::VariableDefinition;
use jian_ops_schema::PenDocument;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// A structurally-shared mirror of a [`PenDocument`] for undo history.
///
/// The `skeleton` is a full `PenDocument` with every node-children
/// vector emptied (root `children` and each page's `children`); it
/// carries all the cheap scalar / collection fields verbatim. The
/// actual top-level nodes live in `root_children` / `page_children` as
/// shared `Arc`s, parallel to `skeleton.pages`.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedDoc {
    /// The document minus every node-children vector. Cheap to clone
    /// and, being a real `PenDocument`, robust against schema growth.
    skeleton: PenDocument,
    /// Shared top-level nodes of the single-page root `children`.
    root_children: Vec<Arc<PenNode>>,
    /// Shared top-level nodes per page — `page_children[i]` parallels
    /// `skeleton.pages[i]`.
    page_children: Vec<Vec<Arc<PenNode>>>,
}

impl SharedDoc {
    /// Capture `doc` into a shared snapshot, reusing `anchor`'s `Arc`s
    /// for any top-level subtree that is unchanged. `anchor` is the
    /// adjacent history state (see [`crate::state::EditorState::snapshot_for_history_with_anchor`]);
    /// `None` produces all-fresh `Arc`s.
    pub fn capture(doc: &PenDocument, anchor: Option<&SharedDoc>) -> SharedDoc {
        Self::capture_kept(doc, anchor, None)
    }

    /// [`capture`](Self::capture) of the pages `keep` marks only (`None`:
    /// every page); the others keep their metadata in the skeleton and
    /// carry no nodes.
    fn capture_kept(
        doc: &PenDocument,
        anchor: Option<&SharedDoc>,
        keep: Option<&[bool]>,
    ) -> SharedDoc {
        let root_children =
            share_children(&doc.children, anchor.map(|a| a.root_children.as_slice()));
        let mut page_children = Vec::new();
        if let Some(pages) = doc.pages.as_ref() {
            for (i, page) in pages.iter().enumerate() {
                if !page_kept(keep, i) {
                    page_children.push(Vec::new());
                    continue;
                }
                let prev = anchor
                    .and_then(|a| a.page_children.get(i))
                    .map(|v| v.as_slice());
                page_children.push(share_children(&page.children, prev));
            }
        }
        SharedDoc {
            skeleton: strip_children(doc),
            root_children,
            page_children,
        }
    }

    /// [`capture`](Self::capture) against an anchor, with the deep-equal
    /// checks spread over the machine's cores (#352).
    ///
    /// The live MCP server captures the document on the UI thread for
    /// every tool call; on a large unchanged document the compare pass is
    /// the whole cost (~40 ms single-threaded on 184k nodes). Each
    /// top-level node is matched exactly as `capture` matches it (same
    /// slot, else same id, and always deep-equal), so the result is the
    /// same `SharedDoc` with the same reuse decisions. Without an anchor, or on a
    /// single core / wasm, it is `capture`.
    pub fn capture_parallel(doc: &PenDocument, anchor: Option<&SharedDoc>) -> SharedDoc {
        Self::capture_parallel_kept(doc, anchor, None)
    }

    /// [`capture_parallel`](Self::capture_parallel) of the pages whose
    /// `keep` entry is `true` (#456): a request that reads one page shares
    /// that page and leaves the others out, so neither the compare pass on
    /// the UI thread nor the consumer's copy walks them. An index past
    /// `keep` is left out too. The single-page root list is always kept.
    pub fn capture_parallel_pages(
        doc: &PenDocument,
        anchor: Option<&SharedDoc>,
        keep: &[bool],
    ) -> SharedDoc {
        Self::capture_parallel_kept(doc, anchor, Some(keep))
    }

    fn capture_parallel_kept(
        doc: &PenDocument,
        anchor: Option<&SharedDoc>,
        keep: Option<&[bool]>,
    ) -> SharedDoc {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(anchor) = anchor {
            let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
            // One top-level node leaves nothing to split.
            let roots = doc.children.len()
                + doc
                    .pages
                    .iter()
                    .flatten()
                    .map(|p| p.children.len())
                    .sum::<usize>();
            if workers > 1 && roots > 1 {
                return capture_on_workers(doc, anchor, workers.min(MAX_CAPTURE_WORKERS), keep);
            }
        }
        Self::capture_kept(doc, anchor, keep)
    }

    /// Rebuild an owned [`PenDocument`] — deep-clones every shared node
    /// back into place. The materialized document is byte-identical to
    /// the one that was captured.
    pub fn materialize(&self) -> PenDocument {
        let mut doc = self.skeleton.clone();
        doc.children = self
            .root_children
            .iter()
            .map(|a| a.as_ref().clone())
            .collect();
        if let Some(pages) = doc.pages.as_mut() {
            for (page, shared) in pages.iter_mut().zip(self.page_children.iter()) {
                page.children = shared.iter().map(|a| a.as_ref().clone()).collect();
            }
        }
        doc
    }

    /// `self` with the pages `keep` marks taken from `newer` (and `newer`'s
    /// skeleton and root list): the anchor a partial capture leaves behind
    /// (#456), so the next capture of any page still finds what it can
    /// share. Pages `self` has that `newer` left out keep `self`'s nodes;
    /// reuse is always gated by deep equality, so an older entry only costs
    /// a compare.
    pub fn merged_pages(&self, newer: &SharedDoc, keep: &[bool]) -> SharedDoc {
        let page_children = newer
            .page_children
            .iter()
            .enumerate()
            .map(|(index, nodes)| {
                if page_kept(Some(keep), index) {
                    nodes.clone()
                } else {
                    self.page_children.get(index).cloned().unwrap_or_default()
                }
            })
            .collect();
        SharedDoc {
            skeleton: newer.skeleton.clone(),
            root_children: newer.root_children.clone(),
            page_children,
        }
    }

    /// The document with every node-children vector emptied.
    pub(crate) fn skeleton(&self) -> &PenDocument {
        &self.skeleton
    }

    /// The document's variable table, as carried by the skeleton.
    pub fn variables(&self) -> Option<&BTreeMap<String, VariableDefinition>> {
        self.skeleton.variables.as_ref()
    }

    /// Find a node by id within the active page's shared children.
    /// Mirrors the (deliberately un-clamped) snapshot-reader semantics
    /// of the old `snapshot_active_children`: an out-of-range page
    /// index or an empty `pages` list resolves to nothing rather than
    /// falling back to the root `children`.
    pub fn snapshot_find_node(&self, active_page_index: usize, id: &NodeId) -> Option<&PenNode> {
        let children: &[Arc<PenNode>] = match self.skeleton.pages.as_ref() {
            Some(pages) => {
                if pages.get(active_page_index).is_some() {
                    self.page_children
                        .get(active_page_index)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[])
                } else {
                    &[]
                }
            }
            None => &self.root_children,
        };
        find_in_shared(children, id)
    }

    /// Copy-on-write repair used by
    /// `instance_override::repair_scope_snapshots`: for every top-level
    /// entry whose subtree holds a *contaminated* (non-`Ref`) node at
    /// `id`, `Arc::make_mut` the entry — isolating it from any sibling
    /// snapshot that shares the `Arc` — then swap the node for
    /// `replacement`. Entries without contamination keep their shared
    /// `Arc` untouched (their `ptr_eq` sharing is preserved).
    pub(crate) fn repair_swap(&mut self, id: &NodeId, replacement: &PenNode) {
        for entry in &mut self.root_children {
            repair_entry(entry, id, replacement);
        }
        for page in &mut self.page_children {
            for entry in page {
                repair_entry(entry, id, replacement);
            }
        }
    }

    /// Shared top-level nodes of the single-page root. Test / assertion
    /// hook for `ptr_eq` sharing checks.
    pub fn root_children(&self) -> &[Arc<PenNode>] {
        &self.root_children
    }

    /// Shared top-level nodes of page `i`, if any.
    pub fn page_children(&self, i: usize) -> Option<&[Arc<PenNode>]> {
        self.page_children.get(i).map(|v| v.as_slice())
    }
}

impl jian_ops_schema::image_table::SaveImageOrder for SharedDoc {
    fn visit_save_image_sources(&self, visit: &mut dyn FnMut(&jian_ops_schema::node::ImageSrc)) {
        jian_ops_schema::image_table::visit_legacy_node_roots(
            self.root_children.iter().map(Arc::as_ref),
            visit,
        );
        for page in self.page_children.iter().rev() {
            jian_ops_schema::image_table::visit_legacy_node_roots(
                page.iter().map(Arc::as_ref),
                visit,
            );
        }
    }
}

/// Serialize the shared snapshot directly as the canonical `PenDocument`
/// wire shape. This lets background saves keep unchanged top-level subtrees
/// behind `Arc` instead of materializing a second owned document first.
impl Serialize for SharedDoc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let doc = &self.skeleton;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("version", &doc.version)?;
        serialize_option(&mut map, "name", &doc.name)?;
        serialize_option(&mut map, "themes", &doc.themes)?;
        serialize_option(&mut map, "variables", &doc.variables)?;
        if doc.pages.is_some() {
            map.serialize_entry("pages", &SharedPages(self))?;
        }
        map.serialize_entry("children", &SharedNodes(&self.root_children))?;
        serialize_option(&mut map, "formatVersion", &doc.format_version)?;
        serialize_option(&mut map, "responsive", &doc.responsive)?;
        serialize_option(&mut map, "id", &doc.id)?;
        serialize_option(&mut map, "app", &doc.app)?;
        serialize_option(&mut map, "routes", &doc.routes)?;
        serialize_option(&mut map, "state", &doc.state)?;
        serialize_option(&mut map, "lifecycle", &doc.lifecycle)?;
        serialize_option(&mut map, "logicModules", &doc.logic_modules)?;
        serialize_option(&mut map, "designMd", &doc.design_md)?;
        serialize_option(&mut map, "conversion", &doc.conversion)?;
        map.end()
    }
}

fn serialize_option<M, T>(map: &mut M, key: &'static str, value: &Option<T>) -> Result<(), M::Error>
where
    M: SerializeMap,
    T: Serialize,
{
    if let Some(value) = value {
        map.serialize_entry(key, value)?;
    }
    Ok(())
}

struct SharedNodes<'a>(&'a [Arc<PenNode>]);

impl Serialize for SharedNodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for node in self.0 {
            seq.serialize_element(node.as_ref())?;
        }
        seq.end()
    }
}

struct SharedPages<'a>(&'a SharedDoc);

impl Serialize for SharedPages<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let pages = self.0.skeleton.pages.as_deref().unwrap_or_default();
        let mut seq = serializer.serialize_seq(Some(pages.len()))?;
        for (index, page) in pages.iter().enumerate() {
            let children = self
                .0
                .page_children
                .get(index)
                .map(Vec::as_slice)
                .unwrap_or_default();
            seq.serialize_element(&SharedPage { page, children })?;
        }
        seq.end()
    }
}

struct SharedPage<'a> {
    page: &'a PenPage,
    children: &'a [Arc<PenNode>],
}

impl Serialize for SharedPage<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.page.id)?;
        map.serialize_entry("name", &self.page.name)?;
        map.serialize_entry("children", &SharedNodes(self.children))?;
        serialize_option(&mut map, "backgroundColor", &self.page.background_color)?;
        serialize_option(&mut map, "state", &self.page.state)?;
        serialize_option(&mut map, "lifecycle", &self.page.lifecycle)?;
        map.end()
    }
}

/// Whether page `index` is captured under `keep` (`None`: every page).
fn page_kept(keep: Option<&[bool]>, index: usize) -> bool {
    keep.is_none_or(|keep| keep.get(index).copied().unwrap_or(false))
}

/// Component prototypes stored in a snapshot, shared by `Arc`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SharedComponents {
    components: Vec<Arc<Component>>,
    document_backing: DocumentComponentBacking,
}

impl SharedComponents {
    /// Capture the library, reusing `anchor`'s `Arc`s for unchanged
    /// prototypes (id + deep-equality gated).
    pub fn capture(lib: &ComponentLibrary, anchor: Option<&SharedComponents>) -> Self {
        SharedComponents {
            components: share_components(&lib.components, anchor.map(|a| a.components.as_slice())),
            document_backing: lib.document_backing(),
        }
    }

    /// Rebuild an owned [`ComponentLibrary`].
    pub fn materialize(&self) -> ComponentLibrary {
        let mut library = ComponentLibrary::default();
        library.components = self.components.iter().map(|a| a.as_ref().clone()).collect();
        library.restore_document_backing(Arc::clone(&self.document_backing));
        library
    }

    /// Shared prototypes. Test / assertion hook for `ptr_eq` checks.
    pub fn components(&self) -> &[Arc<Component>] {
        &self.components
    }
}

// --- Free helpers ----------------------------------------------------

/// A `PenDocument` with every node-children vector emptied — the
/// snapshot skeleton. Written as an explicit struct literal (no
/// `..Default`) so a new `PenDocument` field fails to compile here
/// until it is classified, keeping the skeleton schema-complete.
fn strip_children(doc: &PenDocument) -> PenDocument {
    PenDocument {
        version: doc.version.clone(),
        name: doc.name.clone(),
        themes: doc.themes.clone(),
        variables: doc.variables.clone(),
        pages: doc
            .pages
            .as_ref()
            .map(|pages| pages.iter().map(strip_page_children).collect()),
        children: Vec::new(),
        format_version: doc.format_version.clone(),
        id: doc.id.clone(),
        app: doc.app.clone(),
        routes: doc.routes.clone(),
        state: doc.state.clone(),
        lifecycle: doc.lifecycle.clone(),
        logic_modules: doc.logic_modules.clone(),
        design_md: doc.design_md.clone(),
        conversion: doc.conversion.clone(),
        motion: doc.motion,
        // The responsive schema opt-in is document-level metadata, not
        // node content — survives the children-strip like every other
        // metadata field above.
        responsive: doc.responsive,
    }
}

/// A `PenPage` with its `children` emptied. Explicit literal, same
/// schema-completeness rationale as [`strip_children`].
fn strip_page_children(page: &PenPage) -> PenPage {
    PenPage {
        id: page.id.clone(),
        name: page.name.clone(),
        background_color: page.background_color.clone(),
        state: page.state.clone(),
        lifecycle: page.lifecycle.clone(),
        children: Vec::new(),
    }
}

/// Upper bound on [`SharedDoc::capture_parallel`]'s threads: the compare
/// is memory-bound, and more threads mostly add spawn cost.
#[cfg(not(target_arch = "wasm32"))]
const MAX_CAPTURE_WORKERS: usize = 8;

/// [`SharedDoc::capture_parallel`]'s body: workers pull top-level nodes
/// one at a time from a shared counter (boards differ wildly in size, so
/// fixed chunks would leave cores idle) and match each against the
/// anchor with [`reuse_or_clone`], the same rule `share_children` uses.
///
/// The calling thread works the queue too, so a helper the OS refuses to
/// start only costs speed: the capture still completes on the UI thread
/// instead of panicking there.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn capture_on_workers(
    doc: &PenDocument,
    anchor: &SharedDoc,
    workers: usize,
    keep: Option<&[bool]>,
) -> SharedDoc {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    // (current top-level nodes, the anchor's) for the root, then each page.
    let mut lists: Vec<(&[PenNode], &[Arc<PenNode>])> =
        vec![(doc.children.as_slice(), anchor.root_children.as_slice())];
    for (i, page) in doc.pages.iter().flatten().enumerate() {
        if !page_kept(keep, i) {
            lists.push((&[], &[]));
            continue;
        }
        let prev = anchor.page_children.get(i).map_or(&[][..], Vec::as_slice);
        lists.push((page.children.as_slice(), prev));
    }
    // Id-keyed fallback maps, built on first miss like `share_children`'s,
    // so an unchanged document never hashes its ids.
    let by_id: Vec<OnceLock<HashMap<&str, &Arc<PenNode>>>> =
        lists.iter().map(|_| OnceLock::new()).collect();
    let slots: Vec<(usize, usize)> = lists
        .iter()
        .enumerate()
        .flat_map(|(l, (current, _))| (0..current.len()).map(move |i| (l, i)))
        .collect();
    let results: Vec<OnceLock<Arc<PenNode>>> = slots.iter().map(|_| OnceLock::new()).collect();
    let next = AtomicUsize::new(0);
    let work = || loop {
        let k = next.fetch_add(1, Ordering::Relaxed);
        let Some(&(l, i)) = slots.get(k) else {
            break;
        };
        let (current, prev) = lists[l];
        let node = &current[i];
        let shared = reuse_or_clone(node, prev.get(i), || {
            by_id[l]
                .get_or_init(|| prev.iter().map(|a| (a.id_str(), a)).collect())
                .get(node.id_str())
                .copied()
        });
        let _ = results[k].set(shared);
    };
    std::thread::scope(|scope| {
        for _ in 1..workers.min(slots.len()) {
            // A refused spawn leaves its share to the others.
            let _ = std::thread::Builder::new().spawn_scoped(scope, work);
        }
        work();
    });
    let mut results = results.into_iter().map(|slot| {
        slot.into_inner()
            .expect("every slot is filled before the scope ends")
    });
    let mut take = |len: usize| results.by_ref().take(len).collect::<Vec<_>>();
    let root_children = take(lists[0].0.len());
    let page_children = lists[1..]
        .iter()
        .map(|(current, _)| take(current.len()))
        .collect();
    SharedDoc {
        skeleton: strip_children(doc),
        root_children,
        page_children,
    }
}

/// The anchor's `Arc` for `node` when one is deep-equal — first the one at
/// the same slot with the same id, then the one `by_id` finds — else a
/// fresh clone. The equality check gates every reuse.
fn reuse_or_clone<'a>(
    node: &PenNode,
    at_slot: Option<&'a Arc<PenNode>>,
    by_id: impl FnOnce() -> Option<&'a Arc<PenNode>>,
) -> Arc<PenNode> {
    if let Some(prev) = at_slot {
        if prev.id_str() == node.id_str() && prev.as_ref() == node {
            return Arc::clone(prev);
        }
    }
    match by_id() {
        Some(prev) if prev.as_ref() == node => Arc::clone(prev),
        _ => Arc::new(node.clone()),
    }
}

/// Build the shared top-level vector for `current`, reusing `prev`'s
/// `Arc`s for equal entries. Index fast path (same slot + same id +
/// deep-equal) with a lazily-allocated id-keyed fallback map. The
/// equality check gates every reuse, so a shared `Arc` is guaranteed
/// content-identical to a fresh clone.
fn share_children(current: &[PenNode], prev: Option<&[Arc<PenNode>]>) -> Vec<Arc<PenNode>> {
    let Some(prev) = prev else {
        return current.iter().map(|n| Arc::new(n.clone())).collect();
    };
    let mut id_map: Option<HashMap<&str, &Arc<PenNode>>> = None;
    current
        .iter()
        .enumerate()
        .map(|(i, node)| {
            // Index fast path, then the id-keyed fallback (allocates a
            // small map — the allocation-free claim applies to the index
            // walk only).
            reuse_or_clone(node, prev.get(i), || {
                let map =
                    id_map.get_or_insert_with(|| prev.iter().map(|a| (a.id_str(), a)).collect());
                map.get(node.id_str()).copied()
            })
        })
        .collect()
}

/// [`share_children`] for component prototypes, keyed by `Component::id`.
fn share_components(current: &[Component], prev: Option<&[Arc<Component>]>) -> Vec<Arc<Component>> {
    let Some(prev) = prev else {
        return current.iter().map(|c| Arc::new(c.clone())).collect();
    };
    let mut id_map: Option<HashMap<&NodeId, &Arc<Component>>> = None;
    current
        .iter()
        .enumerate()
        .map(|(i, comp)| {
            if let Some(prev_arc) = prev.get(i) {
                if prev_arc.id == comp.id && prev_arc.as_ref() == comp {
                    return Arc::clone(prev_arc);
                }
            }
            let map = id_map.get_or_insert_with(|| prev.iter().map(|a| (&a.id, a)).collect());
            if let Some(&prev_arc) = map.get(&comp.id) {
                if prev_arc.as_ref() == comp {
                    return Arc::clone(prev_arc);
                }
            }
            Arc::new(comp.clone())
        })
        .collect()
}

/// Find a node by id among top-level shared `Arc`s (recursing into the
/// owned subtree of each entry).
fn find_in_shared<'a>(children: &'a [Arc<PenNode>], id: &NodeId) -> Option<&'a PenNode> {
    for arc in children {
        let node = arc.as_ref();
        if node.id_str() == id.as_str() {
            return Some(node);
        }
        if let Some(sub) = node.children() {
            if let Some(found) = crate::walkers::find_node(sub, id) {
                return Some(found);
            }
        }
    }
    None
}

/// COW-repair one top-level entry. `Arc::make_mut` runs only when the
/// entry actually holds a non-`Ref` node at `id`.
fn repair_entry(entry: &mut Arc<PenNode>, id: &NodeId, replacement: &PenNode) {
    let needs = {
        let node = entry.as_ref();
        let found = if node.id_str() == id.as_str() {
            Some(node)
        } else {
            node.children()
                .and_then(|c| crate::walkers::find_node(c, id))
        };
        matches!(found, Some(n) if !matches!(n, PenNode::Ref(_)))
    };
    if !needs {
        return;
    }
    let node = Arc::make_mut(entry);
    let target = if node.id_str() == id.as_str() {
        Some(node)
    } else {
        node.children_mut()
            .and_then(|c| crate::walkers::find_node_mut(c, id))
    };
    if let Some(t) = target {
        if !matches!(t, PenNode::Ref(_)) {
            *t = replacement.clone();
        }
    }
}
