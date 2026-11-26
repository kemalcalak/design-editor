//! Component instances as the namer reads them. A `ref` shows its master's
//! content, so a card instance holds the texts that make it a card and
//! name it. This crate cannot reach the component registry; the caller
//! hands an [`InstanceSource`] that expands an instance (the editor core
//! materializes it), and [`NamingPage`] reads the page with every instance
//! expanded. Only the instance itself is ever proposed a name: its content
//! belongs to the master.

use std::collections::HashSet;

use jian_ops_schema::node::PenNode;

use crate::node_mut::{base_mut, children_mut};
use crate::node_util::{base, node_id};

/// Expansions stop this deep, so a master that (through others) holds an
/// instance of itself cannot loop.
const MAX_NESTED_INSTANCES: usize = 8;

/// Expands a component instance into the content it shows.
pub trait InstanceSource {
    /// `instance` (a `ref`) as its master with the instance's overrides,
    /// or `None` when its master is gone or it is no instance.
    fn expand(&self, instance: &PenNode) -> Option<PenNode>;
}

/// No components: instances stay unread (and unnamed).
pub struct NoInstances;

impl InstanceSource for NoInstances {
    fn expand(&self, _instance: &PenNode) -> Option<PenNode> {
        None
    }
}

/// A page's top-level layers as the namer reads them: every instance the
/// source can expand replaced by its content, keeping the instance's own
/// id and name.
pub struct NamingPage {
    roots: Vec<PenNode>,
    instances: HashSet<String>,
}

impl NamingPage {
    /// Read `roots` with the instances `source` expands.
    pub fn new(roots: &[PenNode], source: &dyn InstanceSource) -> Self {
        let mut roots = roots.to_vec();
        let mut instances = HashSet::new();
        for root in &mut roots {
            expand_in(root, source, 0, &mut Some(&mut instances));
        }
        Self { roots, instances }
    }

    /// The page's top-level layers, instances expanded.
    pub fn roots(&self) -> &[PenNode] {
        &self.roots
    }

    /// Whether `node` is an expanded instance of the page (its content is
    /// the master's and is never named).
    pub(crate) fn is_instance(&self, node: &PenNode) -> bool {
        self.instances.contains(node_id(node))
    }
}

/// Expand the instances in `node`'s subtree. `page_instances` records the
/// page's own instances (`None` inside an expansion, whose instances
/// belong to a master).
fn expand_in(
    node: &mut PenNode,
    source: &dyn InstanceSource,
    depth: usize,
    page_instances: &mut Option<&mut HashSet<String>>,
) {
    if matches!(node, PenNode::Ref(_)) {
        if depth >= MAX_NESTED_INSTANCES {
            return;
        }
        let Some(mut expanded) = source.expand(node) else {
            return;
        };
        let own = base(node);
        let (id, name) = (own.id.clone(), own.name.clone());
        let target = base_mut(&mut expanded);
        target.id = id.clone();
        target.name = name;
        if let Some(instances) = page_instances {
            instances.insert(id.to_string());
        }
        expand_in(&mut expanded, source, depth + 1, &mut None);
        *node = expanded;
        return;
    }
    if let Some(kids) = children_mut(node) {
        for kid in kids {
            expand_in(kid, source, depth, page_instances);
        }
    }
}
