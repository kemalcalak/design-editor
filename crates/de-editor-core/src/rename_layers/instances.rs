//! Component instances for the rule-based namer: `de-design-lint` cannot
//! reach the component registry, so the editor hands it this source, which
//! materializes an instance the way detach does (master + overrides).

use de_design_lint::{InstanceSource, NamingPage};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::ref_resolve::{find_component_node, materialize_instance};
use crate::EditorState;

/// Expands the instances of one document from its own masters.
pub struct DocumentInstances<'a> {
    doc: &'a PenDocument,
}

impl<'a> DocumentInstances<'a> {
    pub fn new(doc: &'a PenDocument) -> Self {
        Self { doc }
    }

    /// `roots` (a page's top-level layers of this document) as the namer
    /// reads them, instances expanded.
    pub fn naming_page(&self, roots: &[PenNode]) -> NamingPage {
        NamingPage::new(roots, self)
    }
}

impl InstanceSource for DocumentInstances<'_> {
    fn expand(&self, instance: &PenNode) -> Option<PenNode> {
        let PenNode::Ref(reference) = instance else {
            return None;
        };
        let master = find_component_node(self.doc, &reference.target)?;
        materialize_instance(instance, &master)
    }
}

impl EditorState {
    /// The active page as the layer namer reads it, instances expanded.
    pub fn naming_page(&self) -> NamingPage {
        DocumentInstances::new(&self.doc).naming_page(self.active_children())
    }
}
