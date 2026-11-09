//! Preview pages take input like embedded pages (#324).
//!
//! In Preview (▶) every live browser page the last preview frame showed
//! (`browser_preview`) is directly interactive, with no double-click and
//! no mode: a pointer, wheel or press over the page's box goes to the
//! page, in its CSS px (`PageMapping` over the box on screen, so the
//! preview's camera or device frame enters through where the page was
//! painted). A press on a page keeps the pointer until its release and
//! gives that page the keyboard; a press anywhere else, or Escape, takes
//! the keyboard back, and the preview handles the event as usual. A page
//! still loading (no live frame yet) does not take input. The host keeps
//! its own chrome (switchers, browser bar, floating chrome) in front by
//! not offering events over it.

use de_editor_core::browser_input::{BrowserInputEvent, InputModifiers, MouseButton};
use de_editor_core::browser_interact::PageMapping;
use de_editor_core::{EditorState, NodeId};

use super::{BrowserInteraction, KeyInput, KeyOutcome};
use crate::widgets::browser_preview::{self, PreviewPage};
use crate::Point2D;

impl BrowserInteraction {
    /// Whether the pointer at screen `(x, y)` is over a page the preview
    /// drives (the host shows a plain arrow there).
    pub fn preview_over_page(&self, state: &EditorState, x: f32, y: f32) -> bool {
        self.preview_point(state, x, y).is_some()
    }

    /// The preview page with the keyboard.
    pub fn preview_focus(&self) -> Option<&str> {
        self.preview_focus.as_deref()
    }

    /// A pointer move in Preview. True when it went to a page.
    pub fn preview_pointer_move(
        &mut self,
        state: &EditorState,
        x: f32,
        y: f32,
        modifiers: InputModifiers,
    ) -> bool {
        let Some((node, (px, py), _)) = self.preview_point(state, x, y) else {
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

    /// A mouse button in Preview. True when a page took it; a press off
    /// every page takes the keyboard back and is the preview's.
    #[allow(clippy::too_many_arguments)]
    pub fn preview_pointer_button(
        &mut self,
        state: &EditorState,
        pressed: bool,
        button: MouseButton,
        x: f32,
        y: f32,
        modifiers: InputModifiers,
        now_ms: u64,
    ) -> bool {
        if !pressed {
            if self.held & button.bit() == 0 {
                return false;
            }
            // A release goes to the page wherever it lands.
            let Some((node, (px, py), _)) = self.preview_point(state, x, y) else {
                // The page left the screen mid-press: it still gets its
                // release, where it last saw the pointer.
                self.release_held();
                return true;
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
            return true;
        }
        let Some((node, point, _)) = self.preview_point(state, x, y) else {
            self.preview_focus = None;
            return false;
        };
        self.preview_focus = Some(node.clone());
        self.preview_owns = true;
        self.press_on_page(node, button, point, modifiers, now_ms);
        true
    }

    /// A wheel or trackpad scroll in Preview; `dx` / `dy` move content
    /// like a pan, in screen px. True when it scrolled a page (one screen
    /// px per screen px) instead of the preview.
    pub fn preview_wheel(
        &mut self,
        state: &EditorState,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        modifiers: InputModifiers,
    ) -> bool {
        let Some((node, (px, py), mapping)) = self.preview_point(state, x, y) else {
            return false;
        };
        let (delta_x, delta_y) = mapping.page_scroll(dx, dy, 1.0);
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

    /// A key in Preview: the focused page gets it. Escape takes the
    /// keyboard back (the page never sees it, a second Escape is the
    /// preview's).
    pub fn preview_key(
        &mut self,
        state: &EditorState,
        input: KeyInput,
        pressed: bool,
    ) -> KeyOutcome {
        let Some(node) = self.focused_preview_page(state) else {
            return KeyOutcome::Editor;
        };
        if input.key == "Escape" {
            if pressed {
                self.preview_focus = None;
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
        self.queue(node, event);
        KeyOutcome::Page
    }

    /// Committed IME text in Preview. True when the focused page took it.
    pub fn preview_insert_text(&mut self, state: &EditorState, text: &str) -> bool {
        let Some(node) = self.focused_preview_page(state) else {
            return false;
        };
        if !text.is_empty() {
            self.queue(
                node,
                BrowserInputEvent::InsertText {
                    text: text.to_string(),
                },
            );
        }
        true
    }

    /// Once per frame: when the preview closed (or started presenting a
    /// deck, whose slides take no page input), release what its pages
    /// hold and drop the keyboard focus. Only what a preview press took
    /// is released: the canvas interaction mode shares the button state.
    /// True when something ended.
    pub fn sync_preview(&mut self, state: &EditorState) -> bool {
        if state.editor_ui.preview.mode && state.preview_slideshow().is_none() {
            return false;
        }
        if !self.preview_owns {
            return false;
        }
        browser_preview::clear();
        self.preview_owns = false;
        self.preview_focus = None;
        self.end();
        true
    }

    /// The focused page, while the preview still shows it live.
    fn focused_preview_page(&self, state: &EditorState) -> Option<String> {
        let node = self.preview_focus.as_ref()?;
        let page = browser_preview::page(node).filter(|page| page.live)?;
        drivable(state, &page)?;
        Some(page.node_id)
    }

    /// The page under screen `(x, y)` (or holding the pointer), the page
    /// point there and the page's mapping.
    fn preview_point(
        &self,
        state: &EditorState,
        x: f32,
        y: f32,
    ) -> Option<(String, (f64, f64), PageMapping)> {
        let page = if self.held != 0 {
            browser_preview::page(self.node.as_deref()?)?
        } else {
            browser_preview::page_at(Point2D::new(x, y)).filter(|page| page.live)?
        };
        let mapping = drivable(state, &page)?;
        Some((page.node_id, mapping.page_point(x, y), mapping))
    }
}

/// How screen points map onto `page`, when the preview may drive it.
fn drivable(state: &EditorState, page: &PreviewPage) -> Option<PageMapping> {
    let layout = state.preview_page_layout(&NodeId::new(&page.node_id))?;
    let rect = page.rect;
    PageMapping::new(
        [rect.origin.x, rect.origin.y, rect.size.x, rect.size.y],
        &layout,
    )
}

#[cfg(test)]
#[path = "browser_interact_preview_tests.rs"]
mod tests;
