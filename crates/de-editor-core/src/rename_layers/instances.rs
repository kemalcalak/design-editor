//! Component instances for the rule-based namer: `de-design-lint` cannot
//! reach the component registry, so the editor hands it this source, which
//! materializes an instance the way detach does (master + overrides).

use std::cell::RefCell;
use std::collections::HashMap;

use de_design_lint::{ExpandedInstances, InstanceSource, NamingPage};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::ref_resolve::{find_component_node, materialize_instance};
use crate::EditorState;

/// Expands the instances of one document from its own masters. Each
/// master is looked up once per source (a page of twenty plan-card
/// instances finds the plan card once); each instance is materialized once
/// by the page that reads it.
pub struct DocumentInstances<'a> {
    doc: &'a PenDocument,
    masters: RefCell<HashMap<String, Option<PenNode>>>,
}

impl<'a> DocumentInstances<'a> {
    pub fn new(doc: &'a PenDocument) -> Self {
        Self {
            doc,
            masters: RefCell::new(HashMap::new()),
        }
    }

    /// `roots` (a page's top-level layers of this document) as the namer
    /// reads them, instances expanded.
    pub fn naming_page<'r>(&self, roots: &'r [PenNode]) -> NamingPage<'r> {
        NamingPage::new(roots, self)
    }

    fn master(&self, target: &str) -> Option<PenNode> {
        if let Some(found) = self.masters.borrow().get(target) {
            return found.clone();
        }
        let found = find_component_node(self.doc, target);
        self.masters
            .borrow_mut()
            .insert(target.to_string(), found.clone());
        found
    }
}

impl InstanceSource for DocumentInstances<'_> {
    fn expand(&self, instance: &PenNode) -> Option<PenNode> {
        let PenNode::Ref(reference) = instance else {
            return None;
        };
        let master = self.master(&reference.target)?;
        materialize_instance(instance, &master)
    }
}

impl EditorState {
    /// The active page as the layer namer reads it, instances expanded.
    pub fn naming_page(&self) -> NamingPage<'_> {
        DocumentInstances::new(&self.doc).naming_page(self.active_children())
    }

    /// The expansions of the active page's instances, for a snapshot that
    /// names the page away from the document.
    pub fn active_page_instances(&self) -> ExpandedInstances {
        ExpandedInstances::collect(self.active_children(), &DocumentInstances::new(&self.doc))
    }
}
