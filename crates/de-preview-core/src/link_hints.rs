//! "Where does this go" in Preview (#371).
//!
//! - [`PreviewSession::hovered_link_hint`]: what the clickable under the
//!   pointer does (`→ Pricing`, `Back`, `Opens example.com`); hosts paint
//!   it as a tooltip next to the pointer (`browse_chrome::paint_link_hint`),
//!   unless the browser bar's link-hint button turned it off.
//! - Hotspots: while the host holds Alt / Option, every clickable on the
//!   screen gets a translucent overlay (Figma's hotspot hints), drawn as
//!   locked overlay nodes in the painted scene so every paint path (canvas,
//!   device frame, transitions) shows them and hit-testing ignores them.

use de_editor_core::interaction_edit::{open_url_of, unquote_literal, NavigateVerb};
use de_editor_core::PenNodeExt;
use de_editor_ui::layout_scene::{LayoutScene, NodeKind, SceneNode};
use de_editor_ui::Color;
use jian_ops_schema::node::PenNode;

use crate::PreviewSession;

/// What a clickable does, for its tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewLinkHint {
    /// Goes to the screen named `name` (its route when it has no name).
    Navigate {
        name: String,
    },
    Back,
    /// Opens a web address on `host`.
    OpenUrl {
        host: String,
    },
    /// Scrolls the page to the section named `name`.
    ScrollTo {
        name: String,
    },
}

/// The overlay colour of a hotspot: the canvas link blue, translucent.
const HOTSPOT: Color = Color {
    r: 0.231,
    g: 0.51,
    b: 0.965,
    a: 0.22,
};

/// Suffix of the overlay node ids, so they never collide with a node.
const HOTSPOT_SUFFIX: &str = "#hotspot";

impl PreviewSession {
    /// The hint for the clickable under the (hovering) pointer, from its
    /// tap action (else long press); `None` when nothing clickable is
    /// hovered or its action is not a link.
    pub fn hovered_link_hint(&self) -> Option<PreviewLinkHint> {
        let id = self
            .interaction
            .hovered_node()
            .and_then(|node| self.clickable_ancestor(node))?;
        let node = self.runtime_schema(&id)?;
        let events = node.events()?;
        let action = events
            .on_tap
            .as_ref()
            .or(events.on_long_press.as_ref())?
            .first()?;
        let (key, body) = action.single()?;
        if NavigateVerb::from_key(key).is_some() {
            let route = unquote_literal(body.as_str()?);
            let name = self
                .screen_switcher_entries()
                .into_iter()
                .find(|(path, _)| *path == route)
                .map_or(route, |(_, name)| name);
            return Some(PreviewLinkHint::Navigate { name });
        }
        match key {
            "pop" => Some(PreviewLinkHint::Back),
            "open_url" => Some(PreviewLinkHint::OpenUrl {
                host: host_of(&open_url_of(body)?),
            }),
            "scroll_to" => {
                let target = body.get("target")?.as_str()?;
                let name = self
                    .runtime_schema(target)
                    .and_then(|node| node.base().name.clone())
                    .unwrap_or_else(|| target.to_owned());
                Some(PreviewLinkHint::ScrollTo { name })
            }
            _ => None,
        }
    }

    /// Show every hotspot (the host holds Alt / Option). Returns whether
    /// the state changed, so the host repaints only then.
    pub fn set_show_hotspots(&mut self, show: bool) -> bool {
        let changed = self.show_hotspots != show;
        self.show_hotspots = show;
        changed
    }

    pub fn show_hotspots(&self) -> bool {
        self.show_hotspots
    }

    /// Overlay every clickable of the painted `scene` (a no-op while the
    /// hotspots are off).
    pub(crate) fn apply_hotspots(&self, scene: &mut LayoutScene) {
        if !self.show_hotspots {
            return;
        }
        let index = scene.active_page_index;
        if let Some(page) = scene.pages.get_mut(index) {
            self.overlay_hotspots(&mut page.children);
        }
    }

    /// Insert a hotspot overlay right after each clickable in `nodes`, so
    /// it paints over the node (and its children) in the node's parent.
    fn overlay_hotspots(&self, nodes: &mut Vec<SceneNode>) {
        let mut index = 0;
        while index < nodes.len() {
            self.overlay_hotspots(&mut nodes[index].children);
            let node = &nodes[index];
            if !node.hidden && self.is_clickable_id(&node.id) {
                let mut overlay =
                    SceneNode::leaf(format!("{}{HOTSPOT_SUFFIX}", node.id), NodeKind::Rect);
                overlay.bounds = node.bounds;
                overlay.rotation = node.rotation;
                overlay.corner_radius = node.corner_radius.max(4.0);
                overlay.fill = Some(HOTSPOT);
                overlay.locked = true;
                nodes.insert(index + 1, overlay);
                index += 1;
            }
            index += 1;
        }
    }

    fn is_clickable_id(&self, id: &str) -> bool {
        self.clickable_ancestor(id).as_deref() == Some(id)
    }

    /// The runtime schema node with id `id`.
    fn runtime_schema(&self, id: &str) -> Option<&PenNode> {
        let document = self.runtime.document.as_ref()?;
        let key = document.tree.by_id.get(id)?;
        document.tree.nodes.get(*key).map(|data| &data.schema)
    }
}

/// `https://www.example.com/a` → `example.com`.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_owned()
}

#[cfg(all(test, not(target_os = "windows")))]
#[path = "link_hints_tests.rs"]
mod tests;
