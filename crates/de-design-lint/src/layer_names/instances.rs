//! Component instances as the namer reads them. A `ref` shows its master's
//! content, so a card instance holds the texts that make it a card and
//! name it. This crate cannot reach the component registry; the caller
//! hands an [`InstanceSource`] that expands an instance (the editor core
//! materializes it), and [`NamingPage`] reads the page with every instance
//! expanded. Only the instance itself is ever proposed a name: its content
//! belongs to the master.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use jian_ops_schema::node::PenNode;

use crate::node_mut::{base_mut, children_mut};
use crate::node_util::{base, children, node_id};

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
/// id and name. A page without instances is read in place, not copied.
pub struct NamingPage<'a> {
    roots: Cow<'a, [PenNode]>,
    instances: HashSet<String>,
}

impl<'a> NamingPage<'a> {
    /// Read `roots` with the instances `source` expands.
    pub fn new(roots: &'a [PenNode], source: &dyn InstanceSource) -> Self {
        if !roots.iter().any(holds_instance) {
            return Self {
                roots: Cow::Borrowed(roots),
                instances: HashSet::new(),
            };
        }
        let mut owned = roots.to_vec();
        let mut instances = HashSet::new();
        for root in &mut owned {
            expand_in(root, source, 0, &mut Some(&mut instances));
        }
        Self {
            roots: Cow::Owned(owned),
            instances,
        }
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

/// The expansions of a page's instances, taken once (an MCP snapshot
/// carries them to its worker instead of the whole document) and handed
/// back by instance id.
#[derive(Default)]
pub struct ExpandedInstances(HashMap<String, PenNode>);

impl ExpandedInstances {
    /// Expand every instance under `roots` with `source`.
    pub fn collect(roots: &[PenNode], source: &dyn InstanceSource) -> Self {
        fn walk(node: &PenNode, source: &dyn InstanceSource, out: &mut HashMap<String, PenNode>) {
            if matches!(node, PenNode::Ref(_)) {
                let mut expanded = node.clone();
                expand_in(&mut expanded, source, 0, &mut None);
                if !matches!(expanded, PenNode::Ref(_)) {
                    out.insert(node_id(node).to_string(), expanded);
                }
                return;
            }
            for kid in children(node) {
                walk(kid, source, out);
            }
        }
        let mut out = HashMap::new();
        for root in roots {
            walk(root, source, &mut out);
        }
        Self(out)
    }
}

impl InstanceSource for ExpandedInstances {
    fn expand(&self, instance: &PenNode) -> Option<PenNode> {
        self.0.get(node_id(instance)).cloned()
    }
}

/// Whether a `ref` sits anywhere in `node`'s subtree.
fn holds_instance(node: &PenNode) -> bool {
    matches!(node, PenNode::Ref(_)) || children(node).iter().any(holds_instance)
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
