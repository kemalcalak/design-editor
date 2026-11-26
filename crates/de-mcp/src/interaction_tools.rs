//! Prototype interaction tools (#371): `set_interaction` writes one
//! trigger's actions on a node, `remove_interaction` removes them, and
//! `list_interactions` reads the document's flow graph.
//!
//! The writes are `EditorCommand::Interaction` (core `interaction_write`):
//! only the named trigger changes, the node's other handlers stay, and a
//! node inside a component instance gets an instance override. Actions are
//! given as objects (`{"type":"navigate","to":"/pricing"}`), never as the
//! expression strings the `events` schema stores, and a navigate target is
//! resolved to a route here: a route, a screen frame id or a frame name;
//! an unmarked frame is marked as a screen in the same undo step. Overlay
//! actions (#372) are built in `interaction_overlay.rs`.
//!
//! The argument helpers return a finished `ToolOutcome::Err`, so the large
//! `Err` is deliberate (as in `instance_tools.rs`).
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use de_editor_core::interaction_edit::{
    navigate_action, open_url_action, pop_action, scroll_to_action_aligned, InteractionTrigger,
    NavigateVerb,
};
use de_editor_core::interaction_graph::{
    flow_graph, resolve_screen_target, screen_choices, FlowGraph, FlowLink,
};
use de_editor_core::screen_marker_edit::{screen_marker_commands, ScreenMarkerEdit};
use de_editor_core::{EditorCommand, EditorState, InteractionCommand, NodeId, PenNodeExt};
use jian_ops_schema::events::{Action, ActionList};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;
use serde_json::{json, Map, Value};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Where agents read the interaction workflow.
const GUIDE: &str = "get_guidelines topic interactivity";

/// What every interaction tool reads: the whole document, since a node
/// and its link targets may sit on any page.
pub struct InteractionSnapshot {
    doc: PenDocument,
}

pub struct SetInteraction(InteractionSnapshot);
pub struct RemoveInteraction(InteractionSnapshot);
pub struct ListInteractions(InteractionSnapshot);

pub fn set_interaction_snapshot(state: &EditorState) -> SetInteraction {
    SetInteraction(InteractionSnapshot::new(state))
}

pub fn remove_interaction_snapshot(state: &EditorState) -> RemoveInteraction {
    RemoveInteraction(InteractionSnapshot::new(state))
}

pub fn list_interactions_snapshot(state: &EditorState) -> ListInteractions {
    ListInteractions(InteractionSnapshot::new(state))
}

/// A located node: its page and what the walk needs to know about it.
struct Located<'a> {
    page_id: String,
    roots: &'a [PenNode],
    /// Inside a component master (`reusable` frame): its navigate targets
    /// may sit on any page, like the master's instances.
    in_master: bool,
}

impl InteractionSnapshot {
    fn new(state: &EditorState) -> Self {
        Self {
            doc: state.doc.clone(),
        }
    }

    /// `(page id, page name, roots)` of every page; a document without
    /// pages is one page with id `0`.
    fn pages(&self) -> Vec<(String, &str, &[PenNode])> {
        match self.doc.pages.as_deref() {
            Some(pages) if !pages.is_empty() => pages
                .iter()
                .map(|p| (p.id.clone(), p.name.as_str(), p.children.as_slice()))
                .collect(),
            _ => vec![("0".to_owned(), "", self.doc.children.as_slice())],
        }
    }

    /// Find the page holding the node `address` names: an authored id, an
    /// instance path (`inst/child`) or a canvas anchor (`inst__child`).
    fn locate(&self, address: &str) -> Option<Located<'_>> {
        let id = NodeId::new_opt(address)?;
        let lead = if address.contains('/') {
            address.split('/').next().unwrap_or(address).to_owned()
        } else if self
            .pages()
            .iter()
            .any(|(_, _, roots)| de_editor_core::walkers::find_node(roots, &id).is_some())
        {
            address.to_owned()
        } else {
            de_editor_core::instance_anchor::locate_anchor(&self.doc, &id)?
                .ref_id
                .as_str()
                .to_owned()
        };
        let lead = NodeId::new_opt(&lead)?;
        for (page_id, _, roots) in self.pages() {
            let Some(path) = path_to(roots, &lead) else {
                continue;
            };
            if address.contains('/')
                && de_editor_core::instance_anchor::locate_document_path(roots, &self.doc, address)
                    .is_none()
            {
                return None;
            }
            let in_master = path
                .iter()
                .any(|node| matches!(node, PenNode::Frame(f) if f.reusable == Some(true)));
            return Some(Located {
                page_id,
                roots,
                in_master,
            });
        }
        None
    }

    fn page_roots(&self, page_id: &str) -> Option<&[PenNode]> {
        self.pages()
            .into_iter()
            .find(|(id, _, _)| id == page_id)
            .map(|(_, _, roots)| roots)
    }
}

/// The nodes from a root down to `id`, if `id` is under `roots`.
fn path_to<'a>(roots: &'a [PenNode], id: &NodeId) -> Option<Vec<&'a PenNode>> {
    for node in roots {
        if node.id_str() == id.as_str() {
            return Some(vec![node]);
        }
        if let Some(mut path) = node.children().and_then(|kids| path_to(kids, id)) {
            path.insert(0, node);
            return Some(path);
        }
    }
    None
}

fn invalid(message: impl Into<String>) -> ToolOutcome {
    ToolOutcome::Err(ToolErrorCode::InvalidArgument, message.into())
}

/// The first alias with a non-blank value.
fn arg<'a>(args: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .filter_map(|key| args.get(*key))
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
}

/// The node a write names: `nodeId` (an id, `instanceId/childId` path or
/// canvas anchor), or `instanceId` plus `childId`.
fn node_address(args: &BTreeMap<String, String>) -> Result<String, ToolOutcome> {
    if let Some(node) = arg(args, &["nodeId", "node_id", "id"]) {
        return Ok(node.to_owned());
    }
    match (
        arg(args, &["instanceId", "instance_id"]),
        arg(args, &["childId", "child_id"]),
    ) {
        (Some(instance), Some(child)) => Ok(format!("{instance}/{child}")),
        (Some(instance), None) => Ok(instance.to_owned()),
        _ => Err(ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "nodeId is required (an id, or instanceId/childId inside an instance)".into(),
        )),
    }
}

fn trigger_arg(args: &BTreeMap<String, String>) -> Result<InteractionTrigger, ToolOutcome> {
    let Some(raw) = arg(args, &["trigger"]) else {
        return Err(ToolOutcome::Err(
            ToolErrorCode::MissingArgument,
            "trigger is required: tap, hover or long_press".into(),
        ));
    };
    InteractionTrigger::from_wire(raw).ok_or_else(|| {
        invalid(format!(
            "trigger must be tap, hover or long_press, got {raw}"
        ))
    })
}

impl SetInteraction {
    fn run(&self, args: &BTreeMap<String, String>) -> Result<ToolOutcome, ToolOutcome> {
        let snapshot = &self.0;
        let address = node_address(args)?;
        let trigger = trigger_arg(args)?;
        let located = snapshot.locate(&address).ok_or_else(|| {
            invalid(format!(
                "no node {address} (use an id, or instanceId/childId inside an instance)"
            ))
        })?;
        let raw = arg(args, &["actions", "action"]).ok_or_else(|| {
            ToolOutcome::Err(
                ToolErrorCode::MissingArgument,
                format!("actions is required: a list of navigate / back / scroll_to / open_url / open_overlay / swap_overlay / close_overlay; see {GUIDE}"),
            )
        })?;
        let items = match serde_json::from_str::<Value>(raw) {
            Ok(Value::Array(items)) => items,
            Ok(item @ Value::Object(_)) => vec![item],
            _ => return Err(invalid("actions must be a JSON array of action objects")),
        };
        if items.is_empty() {
            return Err(invalid(
                "actions is empty; call remove_interaction to remove a trigger",
            ));
        }
        let mut built = ActionList::new();
        let mut marks = Vec::new();
        let mut summary = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let Some(object) = item.as_object() else {
                return Err(invalid(format!("actions[{index}] must be an object")));
            };
            let (action, echo) = self
                .action(&located, object, &mut marks)
                .map_err(|message| invalid(format!("actions[{index}]: {message}")))?;
            built.push(action);
            summary.push(echo);
        }
        let page_id = located.page_id.clone();
        let mut commands: Vec<EditorCommand> = marks;
        commands.push(EditorCommand::Interaction(InteractionCommand::Set {
            node_id: NodeId::new(address.clone()),
            trigger,
            actions: Some(built),
            page_id: Some(page_id.clone()),
        }));
        let command = if commands.len() == 1 {
            commands.pop().expect("one command")
        } else {
            EditorCommand::Batch { commands }
        };
        let result = json!({
            "nodeId": address,
            "pageId": page_id,
            "trigger": trigger.wire_name(),
            "actions": summary,
        });
        Ok(ToolOutcome::OkJsonWithCommand(result.to_string(), command))
    }

    /// Build one action; `marks` collects screen-marker writes a navigate
    /// to an unmarked frame needs.
    fn action(
        &self,
        located: &Located<'_>,
        object: &Map<String, Value>,
        marks: &mut Vec<EditorCommand>,
    ) -> Result<(Action, Value), String> {
        let text = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let kind = text("type").ok_or(
            "type is required: navigate, back, scroll_to, open_url, open_overlay, swap_overlay or close_overlay",
        )?;
        if let Some(built) = crate::interaction_overlay::overlay_action(&self.0.doc, kind, object)?
        {
            return Ok(built);
        }
        match kind {
            "navigate" => {
                let to =
                    text("to").ok_or("navigate needs to: a route, a screen id or a screen name")?;
                let verb = match text("transition").unwrap_or("fade") {
                    "slide" | "push" => NavigateVerb::Push,
                    "fade" | "replace" => NavigateVerb::Replace,
                    other => return Err(format!("transition must be slide or fade, got {other}")),
                };
                let (target, page_id) = self.screen(located, to)?;
                if target.needs_marker {
                    let roots = self.0.page_roots(&page_id).unwrap_or_default();
                    let commands =
                        screen_marker_commands(roots, &target.node_id, ScreenMarkerEdit::Mark)
                            .map_err(|error| error.to_string())?;
                    marks.extend(
                        commands
                            .into_iter()
                            .map(|command| on_page(command, &page_id)),
                    );
                }
                let echo = json!({
                    "type": "navigate",
                    "to": target.route,
                    "screen": target.name,
                    "screenId": target.node_id,
                    "transition": if verb == NavigateVerb::Push { "slide" } else { "fade" },
                    "markedScreen": target.needs_marker,
                });
                Ok((navigate_action(verb, &target.route), echo))
            }
            "back" => Ok((pop_action(), json!({ "type": "back" }))),
            "scroll_to" => {
                let target =
                    text("target").ok_or("scroll_to needs target: a node id on the same page")?;
                let align = text("align")
                    .or_else(|| text("alignment"))
                    .unwrap_or("start");
                if !matches!(align, "start" | "center" | "end" | "nearest") {
                    return Err(format!(
                        "align must be start, center, end or nearest, got {align}"
                    ));
                }
                let stored = self.scroll_target(located, target)?;
                let echo = json!({ "type": "scroll_to", "target": stored, "align": align });
                Ok((scroll_to_action_aligned(&stored, align), echo))
            }
            "open_url" => {
                let url = text("url").ok_or("open_url needs url")?;
                Ok((
                    open_url_action(url),
                    json!({ "type": "open_url", "url": url }),
                ))
            }
            other => Err(format!(
                "unknown type {other}: navigate, back, scroll_to, open_url, open_overlay, swap_overlay or close_overlay"
            )),
        }
    }

    /// Resolve a navigate target on the node's page (any page for a node
    /// inside a master); refused with the page's screens.
    fn screen(
        &self,
        located: &Located<'_>,
        to: &str,
    ) -> Result<(de_editor_core::interaction_graph::ScreenTarget, String), String> {
        if let Some(target) = resolve_screen_target(located.roots, to) {
            return Ok((target, located.page_id.clone()));
        }
        if located.in_master {
            for (page_id, _, roots) in self.0.pages() {
                if let Some(target) = resolve_screen_target(roots, to) {
                    return Ok((target, page_id));
                }
            }
        }
        let choices = screen_choices(located.roots);
        Err(if choices.is_empty() {
            format!("no screen {to:?}: this page has no top-level frames")
        } else {
            format!(
                "no screen {to:?} on this page; screens: {}",
                choices.join(", ")
            )
        })
    }

    /// The id a `scroll_to` stores: an id on the node's page, or the
    /// canvas anchor of an instance descendant given as a path.
    fn scroll_target(&self, located: &Located<'_>, target: &str) -> Result<String, String> {
        let id = NodeId::new_opt(target).ok_or("target is empty")?;
        if de_editor_core::walkers::find_node(located.roots, &id).is_some() {
            return Ok(target.to_owned());
        }
        let doc = &self.0.doc;
        if target.contains('/') {
            if let Some(hit) =
                de_editor_core::instance_anchor::locate_document_path(located.roots, doc, target)
            {
                return Ok(hit.anchor.as_str().to_owned());
            }
        } else if de_editor_core::instance_anchor::locate_anchor(doc, &id).is_some() {
            return Ok(target.to_owned());
        }
        Err(format!("no node {target} on the node's page"))
    }
}

fn on_page(command: EditorCommand, page_id: &str) -> EditorCommand {
    match command {
        EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            ..
        } => EditorCommand::PatchNodeData {
            node_id,
            patch_json,
            page_id: Some(page_id.to_owned()),
        },
        other => other,
    }
}

impl McpTool for SetInteraction {
    fn name(&self) -> &str {
        "set_interaction"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        self.run(args).unwrap_or_else(|error| error)
    }
}

impl McpTool for RemoveInteraction {
    fn name(&self) -> &str {
        "remove_interaction"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let run = || -> Result<ToolOutcome, ToolOutcome> {
            let address = node_address(args)?;
            let trigger = trigger_arg(args)?;
            let located = self
                .0
                .locate(&address)
                .ok_or_else(|| invalid(format!("no node {address}")))?;
            let mut out = BTreeMap::new();
            out.insert("nodeId".into(), address.clone());
            out.insert("trigger".into(), trigger.wire_name().into());
            Ok(ToolOutcome::OkWithCommand(
                out,
                EditorCommand::Interaction(InteractionCommand::Set {
                    node_id: NodeId::new(address),
                    trigger,
                    actions: None,
                    page_id: Some(located.page_id),
                }),
            ))
        };
        run().unwrap_or_else(|error| error)
    }
}

impl McpTool for ListInteractions {
    fn name(&self) -> &str {
        "list_interactions"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let pages = self.0.pages();
        let page_id = match arg(args, &["pageId", "page_id", "page"]) {
            Some(wanted) => match pages
                .iter()
                .find(|(id, name, _)| id == wanted || *name == wanted)
            {
                Some((id, _, _)) => Some(id.clone()),
                None => return invalid(format!("no page with id or name {wanted}")),
            },
            None => None,
        };
        let mut graph = flow_graph(&self.0.doc, page_id.as_deref());
        if let Some(node) = arg(args, &["nodeId", "node_id"]) {
            graph.retain_node(node);
        }
        ToolOutcome::OkJson(graph_json(&graph).to_string())
    }
}

/// The compact wire shape: screens, working links, broken links.
pub fn graph_json(graph: &FlowGraph) -> Value {
    let screens: Vec<Value> = graph
        .screens
        .iter()
        .map(|s| {
            let mut screen = json!({
                "route": s.route,
                "name": s.name,
                "nodeId": s.node_id,
                "pageId": s.page_id,
            });
            if s.entry {
                screen["entry"] = json!(true);
            }
            screen
        })
        .collect();
    let links: Vec<Value> = graph.working().map(link_json).collect();
    let broken: Vec<Value> = graph
        .broken()
        .map(|link| {
            let mut value = link_json(link);
            value["reason"] = json!(link.broken);
            value
        })
        .collect();
    json!({ "screens": screens, "links": links, "broken": broken })
}

fn link_json(link: &FlowLink) -> Value {
    let mut value = json!({
        "from": link.from,
        "fromName": link.from_name,
        "trigger": link.trigger.wire_name(),
        "action": link.action.wire_name(),
        "pageId": link.page_id,
    });
    let mut put = |key: &str, field: Option<Value>| {
        if let Some(field) = field {
            value[key] = field;
        }
    };
    put("to", link.to.clone().map(Value::from));
    put("transition", link.transition.map(Value::from));
    put("target", link.target_id.clone().map(Value::from));
    put("targetName", link.target_name.clone().map(Value::from));
    put(
        "targetPageId",
        link.target_page_id
            .clone()
            .filter(|page| *page != link.page_id)
            .map(Value::from),
    );
    put("via", link.via.clone().map(Value::from));
    put("overlay", link.overlay.clone());
    put("hidden", link.hidden.then_some(Value::Bool(true)));
    put("master", link.in_master.then_some(Value::Bool(true)));
    value
}

#[cfg(test)]
#[path = "interaction_tools_tests.rs"]
mod tests;
