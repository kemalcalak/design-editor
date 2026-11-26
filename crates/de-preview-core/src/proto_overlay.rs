//! Prototype overlays in Preview (#372): `open_overlay` / `swap_overlay` /
//! `close_overlay` show a frame on top of the current screen.
//!
//! The three actions are registered on the session's own jian action
//! registry (the engine needs no change): executing one only queues an
//! [`OverlayRequest`], which the session applies right after the input,
//! pump or frame that ran it. The runtime's router is wrapped so a `pop`
//! while an overlay is open closes the overlay instead of leaving the
//! screen (Back closes the top overlay first).
//!
//! An overlay's content is laid out once when it opens, from the source
//! document (refs and tokens resolved like the canvas), so a frame on
//! another page or a component instance works. It is never mounted in the
//! runtime: `proto_overlay_input.rs` hit-tests it and runs its nodes' tap /
//! hover actions through the runtime's task queue, and
//! `proto_overlay_scene.rs` composes it (and its dim) into the painted
//! scene, fixed to the screen's viewport.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use async_trait::async_trait;
use de_editor_core::overlay_action::{
    OverlayAction, OverlaySpec, CLOSE_OVERLAY, OPEN_OVERLAY, SWAP_OVERLAY,
};
use de_editor_core::PenNodeExt;
use de_editor_ui::layout_scene::SceneNode;
use de_editor_ui::Rect;
use jian_core::action::action_trait::{ActionImpl, BoxedAction};
use jian_core::action::context::ActionContext;
use jian_core::action::error::{ActionError, ActionResult};
use jian_core::action::services::{RouteState, Router};
use jian_ops_schema::events::ActionList;
use jian_ops_schema::node::PenNode;

use crate::PreviewSession;

/// What an overlay action (or a Back while one is open) asks for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OverlayRequest {
    Action {
        action: OverlayAction,
        /// The node whose handler ran it (a manual overlay's anchor).
        trigger: Option<String>,
    },
    /// Back / `pop` with an overlay open: close the top one.
    Back,
}

type Requests = Rc<RefCell<Vec<OverlayRequest>>>;

/// One open overlay.
#[derive(Debug, Clone)]
pub(crate) struct OverlayEntry {
    pub(crate) spec: OverlaySpec,
    /// Scene-space bottom-left of the trigger, for a manual position.
    pub(crate) trigger_bottom_left: Option<(f32, f32)>,
    /// The overlay's laid-out content, its top-left at (0, 0).
    pub(crate) node: SceneNode,
    /// Tap / hover actions by node id, and each node's parent.
    pub(crate) taps: HashMap<String, ActionList>,
    pub(crate) hovers: HashMap<String, ActionList>,
    pub(crate) parents: HashMap<String, String>,
    pub(crate) opened_at_ms: u64,
    /// How far a tall overlay is scrolled inside itself.
    pub(crate) scroll_y: f32,
}

/// Where a press landed while overlays are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OverlayHit {
    /// Inside the top overlay, on the clickable with this id (if any).
    Inside(Option<String>),
    Outside,
}

/// The session's overlay stack.
pub(crate) struct PrototypeOverlays {
    requests: Requests,
    /// Open overlays, shared with the router wrapper.
    depth: Rc<Cell<usize>>,
    pub(crate) stack: Vec<OverlayEntry>,
    /// Overlays Back closed, newest last: Forward reopens them.
    pub(crate) forward: Vec<OverlayEntry>,
    /// The scene-space rect overlays were last placed in (the device
    /// screen, or the screen board on the canvas).
    pub(crate) viewport: Cell<Option<Rect>>,
    pub(crate) presses: HashMap<u32, OverlayHit>,
    pub(crate) hovered: Option<String>,
    /// Something changed that needs a repaint.
    pub(crate) changed: Cell<bool>,
}

impl Default for PrototypeOverlays {
    fn default() -> Self {
        Self {
            requests: Rc::new(RefCell::new(Vec::new())),
            depth: Rc::new(Cell::new(0)),
            stack: Vec::new(),
            forward: Vec::new(),
            viewport: Cell::new(None),
            presses: HashMap::new(),
            hovered: None,
            changed: Cell::new(false),
        }
    }
}

impl PrototypeOverlays {
    fn sync_depth(&self) {
        self.depth.set(self.stack.len());
        self.changed.set(true);
    }

    pub(crate) fn take_changed(&self) -> bool {
        self.changed.replace(false)
    }

    /// Drop every overlay (a screen switch).
    pub(crate) fn clear(&mut self) {
        if self.stack.is_empty() && self.forward.is_empty() {
            return;
        }
        self.stack.clear();
        self.forward.clear();
        self.presses.clear();
        self.hovered = None;
        self.sync_depth();
    }
}

/// The actions' executable: queue the request.
struct QueueOverlay {
    name: &'static str,
    action: OverlayAction,
    requests: Requests,
}

#[async_trait(?Send)]
impl ActionImpl for QueueOverlay {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn execute(&self, ctx: &ActionContext) -> ActionResult {
        self.requests.borrow_mut().push(OverlayRequest::Action {
            action: self.action.clone(),
            trigger: ctx.node_id.clone(),
        });
        Ok(())
    }
}

/// `pop` closes the top overlay while one is open; everything else goes
/// to the screen router.
struct OverlayRouter {
    inner: Rc<dyn Router>,
    depth: Rc<Cell<usize>>,
    requests: Requests,
}

impl Router for OverlayRouter {
    fn current(&self) -> RouteState {
        self.inner.current()
    }
    fn push(&self, path: &str) {
        self.inner.push(path);
    }
    fn replace(&self, path: &str) {
        self.inner.replace(path);
    }
    fn pop(&self) {
        if self.depth.get() > 0 {
            self.requests.borrow_mut().push(OverlayRequest::Back);
        } else {
            self.inner.pop();
        }
    }
    fn reset(&self, path: &str) {
        self.inner.reset(path);
    }
}

/// Register the three actions, allow them in Preview, and wrap the
/// router. Runs after `effects::install_on_runtime` (which installs the
/// fixed allowlist this widens).
pub(crate) fn install(runtime: &mut jian_core::Runtime, overlays: &PrototypeOverlays) {
    for name in [OPEN_OVERLAY, SWAP_OVERLAY, CLOSE_OVERLAY] {
        let requests = overlays.requests.clone();
        runtime.actions.borrow_mut().register(
            name,
            Box::new(move |body: &serde_json::Value| -> Result<BoxedAction, ActionError> {
                let action = OverlayAction::parse_body(name, body).ok_or_else(|| {
                    ActionError::Custom(format!("{name} needs a target frame"))
                })?;
                Ok(Box::new(QueueOverlay {
                    name,
                    action,
                    requests: requests.clone(),
                }))
            }),
        );
    }
    let allowed = jian_core::action::policy::PreviewActionPolicy::ALLOWED
        .iter()
        .copied()
        .chain([OPEN_OVERLAY, SWAP_OVERLAY, CLOSE_OVERLAY])
        .map(str::to_owned);
    runtime.set_policy(Some(Rc::new(
        jian_core::action::policy::AllowListPolicy::new(allowed),
    )));
    runtime.nav = Rc::new(OverlayRouter {
        inner: runtime.nav.clone(),
        depth: overlays.depth.clone(),
        requests: overlays.requests.clone(),
    });
}

impl PreviewSession {
    /// The per-frame reconcile the host calls before paint: apply queued
    /// overlay requests, then commit a pending screen switch (which
    /// closes every overlay: a navigate from inside one leaves them all).
    /// See [`Self::reconcile_routes`].
    pub fn reconcile(&mut self, now_ms: u64) -> crate::app_mode::ReconcileOutcome {
        let overlays = self.sync_prototype_overlays();
        let mut outcome = self.reconcile_routes(now_ms);
        if outcome.switched {
            self.proto_overlays.clear();
        }
        outcome.repaint |= overlays | self.proto_overlays.take_changed();
        outcome
    }

    /// Whether an overlay is showing.
    pub fn has_open_overlays(&self) -> bool {
        !self.proto_overlays.stack.is_empty()
    }

    /// The ids of the open overlays' frames, bottom first.
    pub fn open_overlay_targets(&self) -> Vec<String> {
        self.proto_overlays
            .stack
            .iter()
            .map(|entry| entry.spec.target.clone())
            .collect()
    }

    /// Close the top overlay (Esc). `false` when none is open.
    pub fn close_top_overlay(&mut self) -> bool {
        let closed = self.proto_overlays.stack.pop().is_some();
        if closed {
            self.proto_overlays.presses.clear();
            self.proto_overlays.hovered = None;
            self.proto_overlays.sync_depth();
        }
        closed
    }

    /// Back with an overlay open: close the top one and remember it for
    /// Forward, like a browser. `false` when none is open.
    pub(crate) fn overlay_back(&mut self) -> bool {
        let Some(entry) = self.proto_overlays.stack.pop() else {
            return false;
        };
        self.proto_overlays.forward.push(entry);
        self.proto_overlays.presses.clear();
        self.proto_overlays.hovered = None;
        self.proto_overlays.sync_depth();
        true
    }

    /// Forward after a Back closed an overlay: reopen it.
    pub(crate) fn overlay_forward(&mut self) -> bool {
        let Some(mut entry) = self.proto_overlays.forward.pop() else {
            return false;
        };
        entry.opened_at_ms = self.last_now_ms;
        self.proto_overlays.stack.push(entry);
        self.proto_overlays.sync_depth();
        true
    }

    /// Apply the queued overlay requests. Returns whether the stack
    /// changed.
    pub(crate) fn sync_prototype_overlays(&mut self) -> bool {
        let requests: Vec<OverlayRequest> =
            std::mem::take(&mut *self.proto_overlays.requests.borrow_mut());
        let mut changed = false;
        for request in requests {
            changed |= match request {
                OverlayRequest::Back => self.overlay_back(),
                OverlayRequest::Action {
                    action: OverlayAction::Close,
                    ..
                } => self.close_top_overlay(),
                OverlayRequest::Action {
                    action: OverlayAction::Open(spec),
                    trigger,
                } => self.open_overlay(spec, trigger.as_deref()),
                OverlayRequest::Action {
                    action:
                        OverlayAction::Swap {
                            target,
                            animation,
                            duration_ms,
                        },
                    trigger,
                } => {
                    let spec = match self.proto_overlays.stack.pop() {
                        Some(top) => OverlaySpec {
                            target,
                            animation,
                            duration_ms,
                            ..top.spec
                        },
                        None => OverlaySpec {
                            animation,
                            duration_ms,
                            ..OverlaySpec::new(target)
                        },
                    };
                    self.proto_overlays.sync_depth();
                    self.open_overlay(spec, trigger.as_deref());
                    true
                }
            };
        }
        changed
    }

    /// Open `spec` on top. Opening a frame that is already open does not
    /// stack it twice; a frame that cannot be found is a warning.
    pub(crate) fn open_overlay(&mut self, spec: OverlaySpec, trigger: Option<&str>) -> bool {
        if self
            .proto_overlays
            .stack
            .iter()
            .any(|entry| entry.spec.target == spec.target)
        {
            return false;
        }
        let Some(content) = self.overlay_content(&spec.target) else {
            self.warnings.push(format!(
                "preview: no frame `{}` to show as an overlay",
                spec.target
            ));
            return false;
        };
        let trigger_bottom_left = trigger.and_then(|id| self.trigger_bottom_left(id));
        let mut entry = OverlayEntry {
            spec,
            trigger_bottom_left,
            node: content.node,
            taps: HashMap::new(),
            hovers: HashMap::new(),
            parents: HashMap::new(),
            opened_at_ms: self.last_now_ms,
            scroll_y: 0.0,
        };
        collect_handlers(&content.source, None, &mut entry);
        self.proto_overlays.stack.push(entry);
        self.proto_overlays.forward.clear();
        self.proto_overlays.presses.clear();
        self.proto_overlays.hovered = None;
        self.proto_overlays.sync_depth();
        // The screen under an overlay is covered: its hover ends.
        self.clear_hover();
        true
    }

    /// The scene-space bottom-left of the node `id`: on the screen, or
    /// inside an open overlay.
    fn trigger_bottom_left(&self, id: &str) -> Option<(f32, f32)> {
        if let Some(node) = self.scene.active_page().and_then(|page| page.find(id)) {
            let b = node.bounds;
            return Some((b.origin.x, b.origin.y + b.size.y));
        }
        let viewport = self.overlay_viewport()?;
        self.proto_overlays.stack.iter().rev().find_map(|entry| {
            let node = entry.node.find(id)?;
            let origin = self.overlay_origin(entry, viewport);
            let b = node.bounds;
            Some((
                origin.0 + b.origin.x,
                origin.1 + b.origin.y + b.size.y - entry.scroll_y,
            ))
        })
    }

    /// Lay out the overlay frame `target` (any page, refs and tokens
    /// resolved like the canvas) with its top-left at (0, 0).
    fn overlay_content(&self, target: &str) -> Option<OverlayContent> {
        let doc = &self.reset_seed.document;
        let (_, node) = de_editor_core::overlay_edit::find_overlay_target(doc, target)?;
        let mut value = serde_json::to_value(node).ok()?;
        value["x"] = serde_json::json!(0);
        value["y"] = serde_json::json!(0);
        if let Some(object) = value.as_object_mut() {
            object.remove("screen");
        }
        let node: PenNode = serde_json::from_value(value).ok()?;
        let roots = de_editor_core::ref_resolve::resolve_refs_for_canvas_roots(
            std::slice::from_ref(&node),
            doc,
        );
        let mut mini = doc.clone();
        mini.pages = None;
        mini.children = roots;
        if de_editor_core::variables_resolve::document_has_tokens(&mini) {
            mini = de_editor_core::variables_resolve::resolve_document_for_canvas(
                &mini,
                &self.reset_seed.active_theme,
            );
        }
        let scene = de_pen_loader::pen_document_to_layout_scene_for_preview(
            &mini,
            &mini,
            self.preserve_authored_geometry,
            &self.reset_seed.active_theme,
            0,
        );
        let node = scene.active_page()?.children.first()?.clone();
        let source = mini.children.into_iter().next()?;
        Some(OverlayContent { node, source })
    }
}

struct OverlayContent {
    node: SceneNode,
    source: PenNode,
}

/// Index the tap / hover actions and parents of an overlay's nodes.
fn collect_handlers(node: &PenNode, parent: Option<&str>, entry: &mut OverlayEntry) {
    let id = node.id_str().to_owned();
    if let Some(parent) = parent {
        entry.parents.insert(id.clone(), parent.to_owned());
    }
    if let Some(events) = node.events() {
        if let Some(list) = events.on_tap.as_ref().filter(|l| !l.is_empty()) {
            entry.taps.insert(id.clone(), list.clone());
        }
        if let Some(list) = events.on_hover_enter.as_ref().filter(|l| !l.is_empty()) {
            entry.hovers.insert(id.clone(), list.clone());
        }
    }
    for child in node.children().into_iter().flatten() {
        collect_handlers(child, Some(&id), entry);
    }
}
