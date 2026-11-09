//! Pointer and key routing of a browser node in interaction mode (#324,
//! phase 4), shared by both hosts.
//!
//! While a node is interactive (`EditorState::interactive_browser`) a host
//! offers each pointer, wheel and key event here first. An event over the
//! node's box (and not over a floating card) becomes a
//! [`BrowserInputEvent`] in the page's CSS px (`PageMapping`) and is
//! queued; the canvas never sees it, so it cannot pan, zoom or drag the
//! node from inside. A press that started on the page keeps the pointer
//! until its release (a drag out of the box still goes to the page). A
//! press anywhere off the page (canvas or chrome), or Escape, leaves the
//! mode; the press then goes on to the editor as usual. A button still
//! held when the mode ends is released on the page. Rotation and flips of
//! the node are not mapped: the page point assumes an upright box. The host drains the queue
//! ([`BrowserInteraction::take`]) and sends it with its own transport
//! (desktop: the node's stream tab over CDP; web: the daemon's
//! `/api/browser/input`).

use de_editor_core::browser_input::{BrowserInputEvent, InputBatch, InputModifiers, MouseButton};
use de_editor_core::browser_interact::{browser_in_hit_path, PageMapping};
use de_editor_core::{EditorState, NodeId};

use crate::layout_scene::LayoutScene;
use crate::widgets::host_canvas_geometry::{canvas_doc_point_unclamped, over_canvas};

/// Two presses of one button this close in time and place are a
/// double-click for the page: the OS default pages are written against
/// (Windows' 500 ms), not the canvas's own 400 ms drill window.
const DOUBLE_CLICK_MS: u64 = 500;
/// How far apart (CSS px) two presses of a double-click may be.
const DOUBLE_CLICK_SLOP: f64 = 4.0;

/// A key event for the page, in DOM terms (see `browser_input`).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyInput {
    pub key: String,
    pub code: String,
    pub text: Option<String>,
    pub modifiers: InputModifiers,
}

/// What became of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// No node is interactive: the editor handles the key.
    Editor,
    /// Taken by the page (queued for it, or swallowed like an Escape
    /// release): the editor must not see it.
    Page,
    /// Escape: the mode ended.
    Left,
}

/// What became of a mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonOutcome {
    /// Not the page's: the editor handles it.
    Editor,
    /// Taken by the page: the editor must not see it.
    Page,
    /// A press outside the node ended the mode; the editor handles it too.
    Left,
}

#[derive(Debug, Clone, Copy)]
struct LastPress {
    at_ms: u64,
    x: f64,
    y: f64,
    button: MouseButton,
    count: u32,
}

/// One host's interaction routing and the events waiting for the page.
#[derive(Debug, Default)]
pub struct BrowserInteraction {
    outbox: InputBatch,
    /// The node the queued events are for.
    node: Option<String>,
    /// Buttons pressed on the page and not released yet.
    held: u8,
    last_press: Option<LastPress>,
    /// Where the page last saw the pointer (CSS px), for releases sent
    /// when the mode ends mid-press.
    last_point: (f64, f64),
}

impl BrowserInteraction {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a press on the page holds the pointer.
    pub fn captures_pointer(&self) -> bool {
        self.held != 0
    }

    /// Double-click on the canvas over `hit_path` (root first): enter
    /// interaction mode on its deepest browser node. True when it did.
    pub fn enter_at(&mut self, state: &mut EditorState, hit_path: &[NodeId]) -> bool {
        let Some(id) = browser_in_hit_path(state, hit_path) else {
            return false;
        };
        if !state.enter_browser_interaction(id) {
            return false;
        }
        self.end();
        true
    }

    /// End the mode when its node lost the selection or its page (hosts
    /// call this once per frame). True when it ended.
    pub fn sync(&mut self, state: &mut EditorState) -> bool {
        if !state.sync_browser_interaction() {
            return false;
        }
        self.end();
        true
    }

    /// The window lost the pointer (focus): release what the page holds,
    /// so it is not left mid-drag.
    pub fn release_pointer(&mut self) {
        self.release_held();
    }

    /// Whether the pointer at screen `(x, y)` is over the interactive
    /// node's page (the host shows a plain arrow there).
    pub fn over_page(
        &self,
        state: &EditorState,
        scene: &LayoutScene,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> bool {
        page_point_at(state, scene, x, y, viewport_w, viewport_h, false).is_some()
    }

    /// A pointer move. True when it went to the page.
    #[allow(clippy::too_many_arguments)]
    pub fn pointer_move(
        &mut self,
        state: &EditorState,
        scene: &LayoutScene,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        let captured = self.captures_pointer();
        let Some((node, _, (px, py))) =
            page_point_at(state, scene, x, y, viewport_w, viewport_h, captured)
        else {
            return false;
        };
        self.queue(
            node,
            BrowserInputEvent::MouseMove {
                x: px,
                y: py,
                buttons: self.held,
                modifiers,
            },
        );
        true
    }

    /// A mouse button. A press outside the interactive node leaves the
    /// mode ([`ButtonOutcome::Left`]).
    #[allow(clippy::too_many_arguments)]
    pub fn pointer_button(
        &mut self,
        state: &mut EditorState,
        scene: &LayoutScene,
        pressed: bool,
        button: MouseButton,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
        now_ms: u64,
    ) -> ButtonOutcome {
        if state.interactive_browser().is_none() {
            self.end();
            return ButtonOutcome::Editor;
        }
        if !pressed {
            if self.held & button.bit() == 0 {
                return ButtonOutcome::Editor;
            }
            // A release goes to the page wherever it lands.
            let Some((node, _, (px, py))) =
                page_point_at(state, scene, x, y, viewport_w, viewport_h, true)
            else {
                self.held &= !button.bit();
                return ButtonOutcome::Page;
            };
            let click_count = self.last_press.map_or(1, |p| p.count);
            self.held &= !button.bit();
            self.queue(
                node,
                BrowserInputEvent::MouseUp {
                    x: px,
                    y: py,
                    button,
                    click_count,
                    buttons: self.held,
                    modifiers,
                },
            );
            return ButtonOutcome::Page;
        }
        let Some((node, _, (px, py))) =
            page_point_at(state, scene, x, y, viewport_w, viewport_h, false)
        else {
            // Any press off the page ends the mode, on the canvas or the
            // chrome alike: keys must not keep going to the page once the
            // user clicked into a panel field.
            state.leave_browser_interaction();
            self.end();
            return ButtonOutcome::Left;
        };
        let count = match self.last_press {
            Some(last)
                if last.button == button
                    && now_ms.saturating_sub(last.at_ms) < DOUBLE_CLICK_MS
                    && (last.x - px).abs() <= DOUBLE_CLICK_SLOP
                    && (last.y - py).abs() <= DOUBLE_CLICK_SLOP =>
            {
                last.count + 1
            }
            _ => 1,
        };
        self.last_press = Some(LastPress {
            at_ms: now_ms,
            x: px,
            y: py,
            button,
            count,
        });
        let held_before = self.held;
        self.held |= button.bit();
        self.queue(
            node,
            BrowserInputEvent::MouseDown {
                x: px,
                y: py,
                button,
                click_count: count,
                buttons: held_before,
                modifiers,
            },
        );
        ButtonOutcome::Page
    }

    /// A wheel or trackpad scroll over the page. `dx` / `dy` move content
    /// like a canvas pan, in screen px. True when it went to the page.
    #[allow(clippy::too_many_arguments)]
    pub fn wheel(
        &mut self,
        state: &EditorState,
        scene: &LayoutScene,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        viewport_w: f32,
        viewport_h: f32,
        modifiers: InputModifiers,
    ) -> bool {
        let captured = self.captures_pointer();
        let Some((node, mapping, (px, py))) =
            page_point_at(state, scene, x, y, viewport_w, viewport_h, captured)
        else {
            return false;
        };
        let (delta_x, delta_y) = mapping.page_scroll(dx, dy, state.viewport.zoom);
        self.queue(
            node,
            BrowserInputEvent::Wheel {
                x: px,
                y: py,
                delta_x,
                delta_y,
                modifiers,
            },
        );
        true
    }

    /// A key press or release. Escape leaves the mode (the page never sees
    /// it); every other key goes to the page while a node is interactive.
    pub fn key(&mut self, state: &mut EditorState, input: KeyInput, pressed: bool) -> KeyOutcome {
        let Some(node) = state.interactive_browser() else {
            return KeyOutcome::Editor;
        };
        if input.key == "Escape" {
            if pressed {
                state.leave_browser_interaction();
                self.end();
                return KeyOutcome::Left;
            }
            return KeyOutcome::Page;
        }
        let event = if pressed {
            BrowserInputEvent::KeyDown {
                key: input.key,
                code: input.code,
                text: input.text.filter(|t| !t.is_empty()),
                modifiers: input.modifiers,
            }
        } else {
            BrowserInputEvent::KeyUp {
                key: input.key,
                code: input.code,
                modifiers: input.modifiers,
            }
        };
        self.queue(node.as_str().to_string(), event);
        KeyOutcome::Page
    }

    /// Committed IME text. True when it went to the page.
    pub fn insert_text(&mut self, state: &EditorState, text: &str) -> bool {
        let Some(node) = state.interactive_browser() else {
            return false;
        };
        if text.is_empty() {
            return true;
        }
        self.queue(
            node.as_str().to_string(),
            BrowserInputEvent::InsertText {
                text: text.to_string(),
            },
        );
        true
    }

    /// The events waiting for the page, with their node. `None` when
    /// nothing waits.
    pub fn take(&mut self) -> Option<(String, Vec<BrowserInputEvent>)> {
        if self.outbox.is_empty() {
            return None;
        }
        let node = self.node.clone()?;
        Some((node, self.outbox.take()))
    }

    /// The mode ended: release what the page holds (queued, so the page is
    /// not left mid-drag) and forget the click history. Events already
    /// queued still go out.
    pub fn end(&mut self) {
        self.release_held();
        self.last_press = None;
    }

    fn release_held(&mut self) {
        let Some(node) = self.node.clone() else {
            self.held = 0;
            return;
        };
        let (x, y) = self.last_point;
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            if self.held & button.bit() == 0 {
                continue;
            }
            self.held &= !button.bit();
            self.queue(
                node.clone(),
                BrowserInputEvent::MouseUp {
                    x,
                    y,
                    button,
                    click_count: 1,
                    buttons: self.held,
                    modifiers: InputModifiers::default(),
                },
            );
        }
    }

    fn queue(&mut self, node: String, event: BrowserInputEvent) {
        if let BrowserInputEvent::MouseMove { x, y, .. }
        | BrowserInputEvent::MouseDown { x, y, .. }
        | BrowserInputEvent::MouseUp { x, y, .. }
        | BrowserInputEvent::Wheel { x, y, .. } = event
        {
            self.last_point = (x, y);
        }
        if self.node.as_deref() != Some(node.as_str()) {
            // Events for a node that is no longer interactive are stale.
            self.outbox.take();
            self.node = Some(node);
        }
        self.outbox.push(event);
    }
}

/// The interactive node's id and page mapping, from its box in `scene`.
pub fn interactive_mapping(
    state: &EditorState,
    scene: &LayoutScene,
) -> Option<(String, PageMapping)> {
    let (id, layout) = state.interactive_browser_layout()?;
    let bounds = scene.active_page()?.find(id.as_str())?.aggregate_bounds();
    let mapping = PageMapping::new(
        [
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.x,
            bounds.size.y,
        ],
        &layout,
    )?;
    Some((id.as_str().to_string(), mapping))
}

/// The interactive node, its mapping and the page point under screen
/// `(x, y)`: only over the node's box on the canvas unless `captured` (a
/// press on the page holds the pointer, wherever it goes).
fn page_point_at(
    state: &EditorState,
    scene: &LayoutScene,
    x: f32,
    y: f32,
    viewport_w: f32,
    viewport_h: f32,
    captured: bool,
) -> Option<(String, PageMapping, (f64, f64))> {
    let (node, mapping) = interactive_mapping(state, scene)?;
    let doc = canvas_doc_point_unclamped(state, x, y);
    let inside = over_canvas(state, x, y, viewport_w, viewport_h) && mapping.contains(doc.x, doc.y);
    (captured || inside).then(|| (node, mapping, mapping.page_point(doc.x, doc.y)))
}

#[cfg(test)]
#[path = "browser_interact_flow_tests.rs"]
mod tests;
