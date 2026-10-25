//! Give id-less nodes a fresh id before typed conversion.
//!
//! `PenNodeBase.id` is required, so a single node without one used to reject
//! the whole document. The repair walks only the node tree (the document's
//! and each page's `children`, recursively), never fills or effects that
//! also carry a `type`. New ids skip every id already in the tree, so they
//! cannot capture a ref target or an instance override path.

use std::collections::HashSet;

use serde_json::{Map, Value};

/// Prefix of the ids handed to nodes that had none.
const MISSING_ID_PREFIX: &str = "missing-id-";

/// Assign a unique id to every node in `root`'s tree that has none. Returns
/// whether anything changed.
pub(super) fn assign_missing_ids(root: &mut Value) -> bool {
    // Detect first, allocation-free: almost every document has no id-less
    // node and pays only this walk.
    let mut missing = false;
    visit_nodes(root, &mut |node| {
        missing |= node.get("id").and_then(Value::as_str).is_none();
    });
    if !missing {
        return false;
    }
    let mut used = HashSet::new();
    visit_nodes(root, &mut |node| {
        if let Some(id) = node.get("id").and_then(Value::as_str) {
            used.insert(id.to_owned());
        }
    });
    let mut next = 1usize;
    visit_nodes(root, &mut |node| {
        if node.get("id").and_then(Value::as_str).is_some() {
            return;
        }
        let id = loop {
            let candidate = format!("{MISSING_ID_PREFIX}{next}");
            next += 1;
            if !used.contains(&candidate) {
                break candidate;
            }
        };
        node.insert("id".to_owned(), Value::String(id));
    });
    true
}

/// Iterative (deep trees must not blow the stack) walk over every node in
/// the document's and the pages' `children` arrays.
/// Visit order is unspecified; callers must not depend on it.
fn visit_nodes(root: &mut Value, visit: &mut dyn FnMut(&mut Map<String, Value>)) {
    let mut stack: Vec<&mut Value> = Vec::new();
    if let Value::Object(doc) = root {
        for (key, value) in doc.iter_mut() {
            match (key.as_str(), value) {
                ("children", Value::Array(children)) => stack.extend(children.iter_mut()),
                ("pages", Value::Array(pages)) => {
                    for page in pages.iter_mut() {
                        if let Some(Value::Array(children)) = page.get_mut("children") {
                            stack.extend(children.iter_mut());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    while let Some(node) = stack.pop() {
        let Value::Object(map) = node else {
            continue;
        };
        visit(map);
        if let Some(Value::Array(children)) = map.get_mut("children") {
            stack.extend(children.iter_mut());
        }
    }
}
