//! Apply linked-library content to the document (#15).
//!
//! Pure state logic: the loader reads the file and harvests a
//! [`LibraryContent`]; this module puts its masters on the hidden
//! `Components` page, replacing only the nodes the link owns, and keeps the
//! library's variables and theme axes in step. See `library_link.rs` for the
//! model.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use jian_ops_schema::node::PenNode;

use crate::library_link::{
    LibraryCommand, LibraryContent, LibraryLink, LibraryPanelAction, LibraryRequest, LibraryStatus,
    LinkedLibrary,
};
use crate::page_mutators::COMPONENTS_PAGE_NAME;
use crate::pen_node_ext::PenNodeExt;
use crate::{ComponentLibrary, EditorState};

impl EditorState {
    /// Apply one [`LibraryCommand`]. Returns whether the document changed
    /// (a status-only update returns `true` too, so the panel repaints).
    pub fn apply_library_command(&mut self, command: LibraryCommand) -> bool {
        match command {
            LibraryCommand::Sync {
                path,
                alias,
                content,
            } => self.sync_library(&path, alias, content),
            LibraryCommand::Unavailable { path, status } => {
                let Some(idx) = self.editor_ui.libraries.position(&path) else {
                    return false;
                };
                self.editor_ui.libraries.entries[idx].status = status;
                true
            }
            LibraryCommand::Unlink { path } => self.unlink_library(&path),
        }
    }

    /// Link `path` (or refresh an existing link) with `content`.
    pub fn sync_library(
        &mut self,
        path: &str,
        alias: Option<String>,
        content: Arc<LibraryContent>,
    ) -> bool {
        let libraries = &mut self.editor_ui.libraries;
        let idx = match libraries.position(path) {
            Some(idx) => idx,
            None => {
                libraries.entries.push(LinkedLibrary {
                    link: LibraryLink::new(path, alias.clone()),
                    ..LinkedLibrary::default()
                });
                libraries.entries.len() - 1
            }
        };
        let entry = &mut libraries.entries[idx];
        if alias.is_some() {
            entry.link.alias = alias;
        }
        entry.content = Some(content);
        self.apply_library_content(idx);
        true
    }

    /// Re-apply every library's cached content. Undo / redo call this so a
    /// restored snapshot cannot bring back an older library version.
    pub fn reconcile_library_masters(&mut self) -> bool {
        let mut changed = false;
        for idx in 0..self.editor_ui.libraries.entries.len() {
            if self.editor_ui.libraries.entries[idx].content.is_some() {
                changed |= self.apply_library_content(idx);
            }
        }
        changed
    }

    /// Remove the link at `path`. Masters an instance still uses stay as
    /// local components; unused ones are dropped. Variables stay.
    pub fn unlink_library(&mut self, path: &str) -> bool {
        let Some(idx) = self.editor_ui.libraries.position(path) else {
            return false;
        };
        let entry = self.editor_ui.libraries.entries.remove(idx);
        let owned: HashSet<String> = entry.link.masters.into_iter().collect();
        let keep = masters_still_used(self, &owned);
        let doomed: HashSet<&String> = owned.iter().filter(|id| !keep.contains(*id)).collect();
        let mut removed = false;
        if let Some(page) = components_page_mut(self) {
            let before = page.children.len();
            page.children
                .retain(|node| !doomed.contains(&node.id_str().to_string()));
            removed = page.children.len() != before;
        }
        if removed {
            self.components = ComponentLibrary::from_document(&self.doc);
            self.mark_document_changed();
        }
        true
    }

    /// Route a Libraries-section press. Link / Reload need file access, so
    /// they queue a request for the host; Unlink applies at once.
    pub fn press_library_action(&mut self, action: LibraryPanelAction) -> bool {
        match action {
            LibraryPanelAction::Link => {
                self.editor_ui.libraries.request = Some(LibraryRequest::Link);
                true
            }
            LibraryPanelAction::Reload => {
                self.editor_ui.libraries.request = Some(LibraryRequest::Reload);
                true
            }
            LibraryPanelAction::Unlink(path) => self.unlink_library(&path),
        }
    }

    /// Put entry `idx`'s cached content into the document. Returns whether
    /// the document changed.
    fn apply_library_content(&mut self, idx: usize) -> bool {
        let Some(content) = self.editor_ui.libraries.entries[idx].content.clone() else {
            return false;
        };
        let owned: HashSet<String> = self.editor_ui.libraries.entries[idx]
            .link
            .masters
            .iter()
            .cloned()
            .collect();

        // Ids the library may not take: everything in the document except
        // the subtrees this link already owns on the Components page.
        let mut foreign: HashSet<String> = self
            .collect_node_ids()
            .into_iter()
            .map(|id| id.as_str().to_string())
            .collect();
        if let Some(page) = components_page(self) {
            for node in page.children.iter().filter(|n| owned.contains(n.id_str())) {
                for id in subtree_ids(node) {
                    foreign.remove(&id);
                }
            }
        }

        let mut accepted: Vec<PenNode> = Vec::with_capacity(content.masters.len());
        let mut conflicts = 0usize;
        let mut seen = HashSet::new();
        for master in &content.masters {
            let ids = subtree_ids(master);
            if ids
                .iter()
                .any(|id| foreign.contains(id) || seen.contains(id))
            {
                conflicts += 1;
                continue;
            }
            seen.extend(ids);
            accepted.push(master.clone());
        }
        let accepted_ids: Vec<String> = accepted.iter().map(|m| m.id_str().to_string()).collect();

        let mut changed = place_masters(self, &owned, accepted);
        changed |= sync_variables(self, idx, &content);
        changed |= add_theme_axes(self, &content);

        let entry = &mut self.editor_ui.libraries.entries[idx];
        entry.link.masters = accepted_ids;
        entry.status = LibraryStatus::Synced {
            components: entry.link.masters.len(),
            conflicts,
        };
        if changed {
            self.components = ComponentLibrary::from_document(&self.doc);
            self.mark_document_changed();
        }
        changed
    }
}

/// Replace the owned masters on the Components page with `accepted`.
fn place_masters(state: &mut EditorState, owned: &HashSet<String>, accepted: Vec<PenNode>) -> bool {
    if components_page(state).is_none() {
        // No page yet: the shared helper creates it (and migrates a
        // single-page document into `pages`) without moving the user.
        return state.append_components_page_masters(accepted) > 0;
    }
    let keep: HashSet<String> = accepted.iter().map(|m| m.id_str().to_string()).collect();
    let Some(page) = components_page_mut(state) else {
        return false;
    };
    let before = page.children.len();
    page.children
        .retain(|node| !owned.contains(node.id_str()) || keep.contains(node.id_str()));
    let mut changed = page.children.len() != before;
    for master in accepted {
        match page
            .children
            .iter_mut()
            .find(|node| node.id_str() == master.id_str())
        {
            Some(slot) if *slot == master => {}
            Some(slot) => {
                *slot = master;
                changed = true;
            }
            None => {
                page.children.push(master);
                changed = true;
            }
        }
    }
    changed
}

/// Library variables the link owns follow the library; a name the document
/// defined itself is never overwritten.
fn sync_variables(state: &mut EditorState, idx: usize, content: &LibraryContent) -> bool {
    let owned: HashSet<String> = state.editor_ui.libraries.entries[idx]
        .link
        .variables
        .iter()
        .cloned()
        .collect();
    let mut now_owned = Vec::new();
    let mut changed = false;
    let vars = state.doc.variables.get_or_insert_with(Default::default);
    for (name, def) in &content.variables {
        match vars.get(name) {
            Some(_) if !owned.contains(name) => continue,
            Some(existing) if existing == def => {}
            _ => {
                vars.insert(name.clone(), def.clone());
                changed = true;
            }
        }
        now_owned.push(name.clone());
    }
    for name in owned.iter().filter(|n| !content.variables.contains_key(*n)) {
        changed |= vars.remove(name).is_some();
    }
    if vars.is_empty() {
        state.doc.variables = None;
    }
    state.editor_ui.libraries.entries[idx].link.variables = now_owned;
    changed
}

/// Theme axes are add-only: an axis the document has stays as it is.
fn add_theme_axes(state: &mut EditorState, content: &LibraryContent) -> bool {
    if content.themes.is_empty() {
        return false;
    }
    let themes = state.doc.themes.get_or_insert_with(Default::default);
    let mut changed = false;
    for (axis, values) in &content.themes {
        if !themes.contains_key(axis) {
            themes.insert(axis.clone(), values.clone());
            changed = true;
        }
    }
    changed
}

fn components_page(state: &EditorState) -> Option<&jian_ops_schema::page::PenPage> {
    state
        .doc
        .pages
        .as_ref()?
        .iter()
        .find(|p| p.name == COMPONENTS_PAGE_NAME)
}

fn components_page_mut(state: &mut EditorState) -> Option<&mut jian_ops_schema::page::PenPage> {
    state
        .doc
        .pages
        .as_mut()?
        .iter_mut()
        .find(|p| p.name == COMPONENTS_PAGE_NAME)
}

fn subtree_ids(node: &PenNode) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        out.push(node.id_str().to_string());
        if let Some(children) = node.children() {
            stack.extend(children.iter());
        }
    }
    out
}

fn collect_ref_targets(node: &PenNode, out: &mut Vec<String>) {
    if let PenNode::Ref(reference) = node {
        out.push(reference.target.clone());
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_ref_targets(child, out);
        }
    }
}

/// Owned master ids an instance outside the owned set still points at,
/// following masters that nest other owned masters.
fn masters_still_used(state: &EditorState, owned: &HashSet<String>) -> HashSet<String> {
    let mut roots: Vec<&PenNode> = state.doc.children.iter().collect();
    if let Some(pages) = state.doc.pages.as_ref() {
        roots.extend(pages.iter().flat_map(|p| p.children.iter()));
    }
    let mut queue: VecDeque<String> = VecDeque::new();
    for node in roots.iter().filter(|n| !owned.contains(n.id_str())) {
        let mut targets = Vec::new();
        collect_ref_targets(node, &mut targets);
        queue.extend(targets.into_iter().filter(|t| owned.contains(t)));
    }
    let mut keep = HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !keep.insert(id.clone()) {
            continue;
        }
        if let Some(master) = roots.iter().find(|n| n.id_str() == id) {
            let mut targets = Vec::new();
            collect_ref_targets(master, &mut targets);
            queue.extend(targets.into_iter().filter(|t| owned.contains(t)));
        }
    }
    keep
}
