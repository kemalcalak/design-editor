//! The document's prototype flow graph (#371): every screen, and every
//! link an authored interaction makes, across all pages.
//!
//! MCP `list_interactions` reports it, and the editor canvas draws its
//! active-page links as arrows (`de-editor-ui` `canvas_prototype_links`).
//!
//! Instances are expanded the way the canvas renders them, so a link
//! authored on a master shows once per instance, from the instance's
//! node (`inst/child`, canvas anchor `inst__child`), and a per-instance
//! override wins. Routes resolve within the link's own page, the way
//! Preview routes: a marked screen (`screen`) first, then an unmarked
//! top-level frame whose name makes that route (Preview's auto-wire
//! routes those too). A link inside a component master resolves against
//! every page, since the master only acts through its instances.

use std::collections::{BTreeMap, HashSet};

use jian_ops_schema::events::Action;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::PenDocument;

use crate::interaction_edit::{open_url_of, unquote_literal, InteractionTrigger, NavigateVerb};
use crate::pen_node_ext::PenNodeExt;
use crate::ref_resolve::resolve_refs_for_canvas_roots;
use crate::screen_marker_edit::route_slug;

/// A routed screen: a top-level frame with a `screen` marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowScreen {
    pub page_id: String,
    pub route: String,
    pub name: String,
    pub node_id: String,
    /// The start screen (`/`).
    pub entry: bool,
}

/// What a link does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowAction {
    Navigate,
    Back,
    ScrollTo,
    OpenUrl,
    /// Any other action key (`set`, `fetch`, …), listed as is.
    Other(String),
}

impl FlowAction {
    /// The MCP spelling.
    pub fn wire_name(&self) -> &str {
        match self {
            Self::Navigate => "navigate",
            Self::Back => "back",
            Self::ScrollTo => "scroll_to",
            Self::OpenUrl => "open_url",
            Self::Other(key) => key,
        }
    }
}

/// One authored action on one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowLink {
    pub page_id: String,
    /// The node, as MCP addresses it (`n42`, or `inst/child` inside an
    /// instance).
    pub from: String,
    /// The node's canvas id (`n42`, or the virtual `inst__child`).
    pub anchor: String,
    pub from_name: String,
    pub trigger: InteractionTrigger,
    pub action: FlowAction,
    /// Route, scroll target, URL or a dynamic expression, as authored.
    pub to: Option<String>,
    /// `slide` (`push`) or `fade` (`replace`) for a navigate.
    pub transition: Option<&'static str>,
    /// Canvas id of the screen frame or scroll target the link reaches.
    pub target_id: Option<String>,
    pub target_name: Option<String>,
    /// The page the target is on, when it resolved.
    pub target_page_id: Option<String>,
    /// `instance <id> (<name>)` for a node inside an instance.
    pub via: Option<String>,
    /// The node or an ancestor is hidden.
    pub hidden: bool,
    /// The node sits inside a component master.
    pub in_master: bool,
    /// The anchors of the node and its ancestors, nearest first.
    pub scope: Vec<String>,
    /// Why the link goes nowhere; `None` for a working link.
    pub broken: Option<String>,
}

/// Every screen and link of the walked pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowGraph {
    pub screens: Vec<FlowScreen>,
    pub links: Vec<FlowLink>,
}

impl FlowGraph {
    /// The links that resolve (or cannot be checked: dynamic routes).
    pub fn working(&self) -> impl Iterator<Item = &FlowLink> {
        self.links.iter().filter(|link| link.broken.is_none())
    }

    /// The links that go nowhere.
    pub fn broken(&self) -> impl Iterator<Item = &FlowLink> {
        self.links.iter().filter(|link| link.broken.is_some())
    }

    /// Keep the links that start at `node` or inside it, or that reach it.
    /// `node` is an authored id, an instance path or a canvas anchor.
    pub fn retain_node(&mut self, node: &str) {
        let prefix = format!("{node}/");
        self.links.retain(|link| {
            link.from == node
                || link.from.starts_with(&prefix)
                || link.scope.iter().any(|id| id == node)
                || link.target_id.as_deref() == Some(node)
        });
    }
}

/// A page as the walk sees it.
struct PageView<'a> {
    id: String,
    name: &'a str,
    roots: &'a [PenNode],
}

fn pages_of(doc: &PenDocument) -> Vec<PageView<'_>> {
    match doc.pages.as_deref() {
        Some(pages) if !pages.is_empty() => pages
            .iter()
            .map(|page| PageView {
                id: page.id.clone(),
                name: page.name.as_str(),
                roots: page.children.as_slice(),
            })
            .collect(),
        _ => vec![PageView {
            id: "0".to_owned(),
            name: "",
            roots: doc.children.as_slice(),
        }],
    }
}

/// Route → (frame id, frame name) for one page: marked screens, then
/// unmarked top-level frames under the route their name makes.
#[derive(Default)]
struct PageRoutes {
    marked: BTreeMap<String, (String, String)>,
    named: BTreeMap<String, (String, String)>,
}

impl PageRoutes {
    fn of(roots: &[PenNode]) -> Self {
        let mut routes = Self::default();
        for node in roots {
            let PenNode::Frame(frame) = node else {
                continue;
            };
            let name = node.base().name.clone().unwrap_or_default();
            let entry = (frame.base.id.clone(), name.clone());
            match frame.screen.as_deref() {
                Some(route) => {
                    routes.marked.entry(route.to_owned()).or_insert(entry);
                }
                None if frame.reusable != Some(true) => {
                    let slug = route_slug(&name);
                    if !slug.is_empty() {
                        routes.named.entry(format!("/{slug}")).or_insert(entry);
                    }
                }
                None => {}
            }
        }
        routes
    }

    fn resolve(&self, route: &str) -> Option<&(String, String)> {
        self.marked.get(route).or_else(|| self.named.get(route))
    }
}

/// Where a `navigate` written by MCP `set_interaction` lands (#371).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenTarget {
    pub route: String,
    pub node_id: String,
    pub name: String,
    /// The frame has no `screen` marker yet: the write marks it (at
    /// `route`) in the same undo step.
    pub needs_marker: bool,
}

/// Resolve `to` among one page's top-level frames: a route (a marked
/// screen, else an unmarked frame whose name makes it), a frame id, or a
/// frame name (trimmed, case-insensitive). An unmarked frame gets the
/// route marking would give it.
pub fn resolve_screen_target(roots: &[PenNode], to: &str) -> Option<ScreenTarget> {
    let to = to.trim();
    let frames: Vec<(&PenNode, Option<&str>)> = roots
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) if frame.reusable != Some(true) => {
                Some((node, frame.screen.as_deref()))
            }
            _ => None,
        })
        .collect();
    let name_of = |node: &PenNode| node.base().name.clone().unwrap_or_default();
    let target = |node: &PenNode, screen: Option<&str>| {
        let route = match screen {
            Some(route) => route.to_owned(),
            None => {
                let mut used = frames
                    .iter()
                    .filter_map(|(_, screen)| screen.map(str::to_owned))
                    .collect();
                crate::screen_marker_edit::unique_route_path(&name_of(node), &mut used)
            }
        };
        ScreenTarget {
            route,
            node_id: node.id_str().to_owned(),
            name: name_of(node),
            needs_marker: screen.is_none(),
        }
    };
    if to.starts_with('/') {
        if let Some((node, screen)) = frames.iter().find(|(_, screen)| *screen == Some(to)) {
            return Some(target(node, *screen));
        }
        let routes = PageRoutes::of(roots);
        let (id, _) = routes.named.get(to)?;
        let (node, screen) = frames.iter().find(|(node, _)| node.id_str() == id)?;
        return Some(target(node, *screen));
    }
    if let Some((node, screen)) = frames.iter().find(|(node, _)| node.id_str() == to) {
        return Some(target(node, *screen));
    }
    let wanted = to.to_lowercase();
    let mut named = frames
        .iter()
        .filter(|(node, _)| name_of(node).trim().to_lowercase() == wanted);
    let (node, screen) = named.next()?;
    // Two frames with that name: ambiguous, so neither.
    if named.next().is_some() {
        return None;
    }
    Some(target(node, *screen))
}

/// The screens a `navigate` can aim at on one page, for an error message:
/// `Name (/route)` for marked screens, `Name (id, not marked yet)` for the
/// other top-level frames.
pub fn screen_choices(roots: &[PenNode]) -> Vec<String> {
    roots
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) if frame.reusable != Some(true) => {
                let name = node.base().name.clone().unwrap_or_default();
                Some(match frame.screen.as_deref() {
                    Some(route) => format!("{name} ({route})"),
                    None => format!("{name} ({}, not marked yet)", frame.base.id),
                })
            }
            _ => None,
        })
        .collect()
}

/// The flow graph of `doc`: every page, or only `page_id` when given.
pub fn flow_graph(doc: &PenDocument, page_id: Option<&str>) -> FlowGraph {
    let pages = pages_of(doc);
    let routes: Vec<PageRoutes> = pages.iter().map(|p| PageRoutes::of(p.roots)).collect();
    let mut graph = FlowGraph::default();
    for (index, page) in pages.iter().enumerate() {
        if page_id.is_some_and(|wanted| wanted != page.id) {
            continue;
        }
        for node in page.roots {
            if let PenNode::Frame(frame) = node {
                if let Some(route) = frame.screen.as_deref() {
                    graph.screens.push(FlowScreen {
                        page_id: page.id.clone(),
                        route: route.to_owned(),
                        name: node.base().name.clone().unwrap_or_default(),
                        node_id: frame.base.id.clone(),
                        entry: route == "/",
                    });
                }
            }
        }
        let expanded = resolve_refs_for_canvas_roots(page.roots, doc);
        let mut ids = HashSet::new();
        collect_ids(&expanded, &mut ids);
        let walk = Walk {
            doc,
            pages: &pages,
            routes: &routes,
            page_index: index,
            ids: &ids,
            expanded: &expanded,
        };
        for node in &expanded {
            walk.node(node, &Context::default(), &mut graph.links);
        }
    }
    graph
}

fn collect_ids<'a>(nodes: &'a [PenNode], out: &mut HashSet<&'a str>) {
    for node in nodes {
        out.insert(node.id_str());
        if let Some(children) = node.children() {
            collect_ids(children, out);
        }
    }
}

fn find_expanded<'a>(nodes: &'a [PenNode], id: &str) -> Option<&'a PenNode> {
    nodes.iter().find_map(|node| {
        if node.id_str() == id {
            Some(node)
        } else {
            node.children().and_then(|kids| find_expanded(kids, id))
        }
    })
}

/// What the walk carries down the tree.
#[derive(Clone, Default)]
struct Context {
    /// The outermost instance this node is inside: its id and name.
    instance: Option<(String, String)>,
    hidden: bool,
    in_master: bool,
    scope: Vec<String>,
}

struct Walk<'a> {
    doc: &'a PenDocument,
    pages: &'a [PageView<'a>],
    routes: &'a [PageRoutes],
    page_index: usize,
    ids: &'a HashSet<&'a str>,
    expanded: &'a [PenNode],
}

impl Walk<'_> {
    fn node(&self, node: &PenNode, parent: &Context, out: &mut Vec<FlowLink>) {
        let id = node.id_str();
        let base = node.base();
        let mut cx = parent.clone();
        cx.hidden |= base.visible == Some(false);
        cx.in_master |= matches!(node, PenNode::Frame(f) if f.reusable == Some(true));
        if cx.instance.is_none() && is_authored_ref(self.doc, id) {
            let name = base.name.clone().unwrap_or_else(|| id.to_owned());
            cx.instance = Some((id.to_owned(), name));
        }
        cx.scope.insert(0, id.to_owned());
        if let Some(handlers) = node.events() {
            for trigger in InteractionTrigger::ALL {
                for action in trigger.actions(handlers).into_iter().flatten() {
                    out.push(self.link(node, &cx, trigger, action));
                }
            }
        }
        for child in node.children().into_iter().flatten() {
            self.node(child, &cx, out);
        }
    }

    fn link(
        &self,
        node: &PenNode,
        cx: &Context,
        trigger: InteractionTrigger,
        action: &Action,
    ) -> FlowLink {
        let page = &self.pages[self.page_index];
        let anchor = node.id_str().to_owned();
        let mut link = FlowLink {
            page_id: page.id.clone(),
            from: self.path_of(&anchor, cx),
            anchor,
            from_name: display_name(node),
            trigger,
            action: FlowAction::Other(String::new()),
            to: None,
            transition: None,
            target_id: None,
            target_name: None,
            target_page_id: None,
            via: cx
                .instance
                .as_ref()
                .filter(|(id, _)| *id != node.id_str())
                .map(|(id, name)| format!("instance {id} ({name})")),
            hidden: cx.hidden,
            in_master: cx.in_master,
            scope: cx.scope.clone(),
            broken: None,
        };
        let Some((key, body)) = action.single() else {
            return link;
        };
        if let Some(verb) = NavigateVerb::from_key(key) {
            link.action = FlowAction::Navigate;
            link.transition = Some(match verb {
                NavigateVerb::Push => "slide",
                NavigateVerb::Replace => "fade",
            });
            self.resolve_route(&mut link, body, cx.in_master);
            return link;
        }
        match key {
            "pop" => link.action = FlowAction::Back,
            "scroll_to" => {
                link.action = FlowAction::ScrollTo;
                let target = body.get("target").and_then(serde_json::Value::as_str);
                match target {
                    Some(target) if self.ids.contains(target) => {
                        link.to = Some(self.path_of(target, cx));
                        link.target_id = Some(target.to_owned());
                        link.target_page_id = Some(page.id.clone());
                        link.target_name = find_expanded(self.expanded, target).map(display_name);
                    }
                    Some(target) => {
                        link.to = Some(target.to_owned());
                        link.broken = Some(format!("no node {target} on this page"));
                    }
                    None => link.broken = Some("scroll_to has no target".to_owned()),
                }
            }
            "open_url" => {
                link.action = FlowAction::OpenUrl;
                link.to = open_url_of(body);
                if link.to.as_deref().is_none_or(str::is_empty) {
                    link.broken = Some("open_url has no address".to_owned());
                }
            }
            other => link.action = FlowAction::Other(other.to_owned()),
        }
        link
    }

    fn resolve_route(&self, link: &mut FlowLink, body: &serde_json::Value, in_master: bool) {
        let Some(source) = body.as_str() else {
            link.to = Some(body.to_string());
            return;
        };
        let route = unquote_literal(source);
        let literal = route != source || source.starts_with('/');
        link.to = Some(route.clone());
        if !literal {
            // An expression: only the runtime knows where it goes.
            return;
        }
        if let Some((id, name)) = self.routes[self.page_index].resolve(&route) {
            link.target_id = Some(id.clone());
            link.target_name = Some(name.clone());
            link.target_page_id = Some(link.page_id.clone());
            return;
        }
        let elsewhere = self
            .routes
            .iter()
            .enumerate()
            .find_map(|(index, routes)| routes.resolve(&route).map(|hit| (index, hit)));
        match elsewhere {
            Some((index, (id, name))) if in_master => {
                link.target_id = Some(id.clone());
                link.target_name = Some(name.clone());
                link.target_page_id = Some(self.pages[index].id.clone());
            }
            Some((index, _)) => {
                link.broken = Some(format!(
                    "the screen with this route is on page {:?}; Preview routes within one page",
                    self.pages[index].name
                ));
            }
            None => link.broken = Some("no screen with this route".to_owned()),
        }
    }

    /// The MCP address of a canvas id: authored ids as they are, a node
    /// inside the instance in `cx` as `instance/child/…`.
    fn path_of(&self, anchor: &str, cx: &Context) -> String {
        anchor_path(
            self.doc,
            anchor,
            cx.instance.as_ref().map(|(id, _)| id.as_str()),
        )
    }
}

/// `inst__a__b` → `inst/a/b` when `instance` is the outermost instance
/// holding it; any other id is returned as is.
fn anchor_path(doc: &PenDocument, anchor: &str, instance: Option<&str>) -> String {
    let Some(instance) = instance else {
        return anchor.to_owned();
    };
    if crate::instance_anchor::find_authored(doc, anchor).is_some() {
        return anchor.to_owned();
    }
    let separator = crate::ref_resolve::instance_child_virtual_id("", "");
    match anchor.strip_prefix(&format!("{instance}{separator}")) {
        Some(rest) => format!(
            "{instance}{}{}",
            crate::instance_descendants::PATH_SEPARATOR,
            rest.replace(
                separator.as_str(),
                &crate::instance_descendants::PATH_SEPARATOR.to_string()
            )
        ),
        None => anchor.to_owned(),
    }
}

fn is_authored_ref(doc: &PenDocument, id: &str) -> bool {
    matches!(
        crate::instance_anchor::find_authored(doc, id),
        Some(PenNode::Ref(_))
    )
}

/// A node's layer name, else a text node's content, else its id.
fn display_name(node: &PenNode) -> String {
    if let Some(name) = node.base().name.as_deref().map(str::trim) {
        if !name.is_empty() {
            return name.to_owned();
        }
    }
    if let PenNode::Text(text) = node {
        if let Ok(serde_json::Value::String(content)) = serde_json::to_value(&text.content) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.chars().take(40).collect();
            }
        }
    }
    node.id_str().to_owned()
}

#[cfg(test)]
#[path = "interaction_graph_tests.rs"]
mod tests;
