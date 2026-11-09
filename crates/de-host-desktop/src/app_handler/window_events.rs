//! Window-lifecycle handlers (close / resize / move / DPI / file drop)
//! for `DesktopApp`'s `window_event` dispatcher. Carved out of the
//! `app_handler.rs` spine to keep it under the 800-line cap; pure code
//! motion.

use crate::{chat_session, image_drop_host, persistence, DesktopApp};
use std::path::PathBuf;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

impl DesktopApp {
    /// The window's close (traffic light or window-strip dot). Closing a
    /// document returns to Home; closing Home itself quits. Cmd+Q
    /// always quits through `MenuAction::Quit`.
    pub(super) fn on_close_requested(&mut self, event_loop: &ActiveEventLoop) {
        // Finalize-lifecycle invariant (0718-1-k3-1 postmortem): the
        // process exiting must not silently drop an in-flight,
        // unfinalized design loop — see `chat_session::
        // finalize_design_session_if_needed`'s doc comment.
        // Best-effort (the worker thread itself is abandoned either
        // way — no graceful async shutdown here), but it must run
        // BEFORE `confirm_close` below, not after: the unsaved-
        // changes prompt's `document_is_dirty` check + its own Save
        // decide what gets persisted, so a finalize mutation applied
        // afterward would never make it to disk.
        chat_session::finalize_design_session_if_needed(
            &mut self.host,
            &self.current_chat,
            "teardown-backstop",
        );
        // The unsaved-changes prompt can abort the close.
        if !self.confirm_close() {
            return;
        }
        self.host.flush_settings_input();
        de_host_services::settings_io::save(self.host.editor_state());
        match crate::app_state::close_target(self.host.home_visible()) {
            crate::app_state::CloseTarget::Quit => event_loop.exit(),
            crate::app_state::CloseTarget::Home => self.close_document_to_home(),
        }
    }

    /// Stop any running turn, swap in a fresh document (File > New's path,
    /// so import sessions reset with it) and show Home over it. The stop
    /// goes through the same drain as the Stop button: otherwise the closed
    /// document's agent would keep streaming into the blank starter behind
    /// Home. Home is not dismissible here: Escape must not land on the
    /// blank starter page as if it were the document just closed.
    fn close_document_to_home(&mut self) {
        self.host.editor_state_mut().chat.pending_stop_chat = true;
        self.drain_stop_chat();
        let outcome = persistence::run_action(
            de_editor_core::editor_ui_state::FileAction::New,
            &mut self.host,
            &mut self.current_path,
            self.window.as_ref(),
        );
        if outcome == de_host_services::doc_io::ActionOutcome::Saved {
            self.mark_document_saved();
        }
        self.host.editor_state_mut().editor_ui.home.show(false);
        self.host.mark_editor_state_dirty();
        self.request_redraw(true);
    }

    pub(super) fn on_resized(&mut self, event_loop: &ActiveEventLoop, size: PhysicalSize<u32>) {
        if !self.apply_window_size(size) {
            return;
        }
        if let Some(ctx) = self.ctx.as_mut() {
            if let Err(err) = ctx.resize(size.width, size.height) {
                eprintln!("design-editor: resize failed: {err}");
                self.error = Some(err);
                event_loop.exit();
            }
        } else {
            self.try_init_render_context(event_loop);
        }
        // Fullscreen enter / exit arrives as a `Resized` on
        // macOS — refresh the flag so the chrome drops its
        // traffic-light reservation when the native lights
        // hide in fullscreen.
        let fullscreen = self
            .window
            .as_ref()
            .is_some_and(|w| w.fullscreen().is_some());
        if self.host.editor_state().editor_ui.window_fullscreen != fullscreen {
            self.host.editor_state_mut().editor_ui.window_fullscreen = fullscreen;
            self.host.mark_editor_state_dirty();
        }
    }

    /// Record a `Resized` size: the logical viewport, the preview layout and
    /// the persisted windowed size, then schedule a repaint. Returns `false`
    /// and changes nothing else for a 0×0 client area,
    /// which Windows reports on minimize (#302): that size is never painted
    /// or saved, so the last real viewport stays and the restore's `Resized`
    /// rebuilds the surface at the real size.
    pub(crate) fn apply_window_size(&mut self, size: PhysicalSize<u32>) -> bool {
        self.window_zero_size = !has_area(size);
        if self.window_zero_size {
            self.pump_browser_live();
            return false;
        }
        self.viewport_width = size.width as f32 / self.dpi;
        self.viewport_height = size.height as f32 / self.dpi;
        // Re-lay-out the preview runtime (if active) against the
        // new canvas region so the live scene tracks the resize.
        self.host
            .preview_resize(self.viewport_width, self.viewport_height);
        // Track geometry for window-state persistence. Only a
        // *windowed* (non-maximized) size is remembered so a
        // restart restores a sensible un-maximize size.
        self.win_maximized = self.window.as_ref().is_some_and(Window::is_maximized);
        if !self.win_maximized {
            self.win_size = Some((size.width, size.height));
        }
        // Any real size repaints, the restore after a minimize included.
        self.request_redraw(true);
        true
    }

    /// Nothing paints while the client area is 0×0 or fully occluded.
    pub(crate) fn surface_hidden(&self) -> bool {
        self.window_zero_size || self.window_occluded
    }

    /// The OS hid or revealed the whole window (macOS minimize, another
    /// Space). Painting pauses while hidden; the reveal repaints.
    pub(crate) fn on_occluded(&mut self, occluded: bool) {
        self.window_occluded = occluded;
        // Nothing paints while hidden, so stop live pages now (#324).
        self.pump_browser_live();
        if !occluded {
            self.request_redraw(true);
        }
    }

    /// The window gained or lost keyboard focus: live pages pause without
    /// it (#324) and resume with it.
    pub(crate) fn on_focused(&mut self, focused: bool) {
        self.window_focused = focused;
        if self.pump_browser_live() || focused {
            self.request_redraw(true);
        }
    }

    /// Start, change or stop live page streams for the last painted frame.
    /// Returns whether the canvas must repaint (a stream stopped and its
    /// snapshot shows again).
    pub(crate) fn pump_browser_live(&mut self) -> bool {
        let active = self.window_focused && !self.surface_hidden();
        let Some(backend) = self.backend.as_mut() else {
            return false;
        };
        self.browser_live.pump(
            self.host.editor_state(),
            self.browser_renders.snapshot_queue(),
            self.dpi,
            active,
            backend,
        )
    }

    pub(super) fn on_moved(&mut self, pos: PhysicalPosition<i32>) {
        // Remember the windowed position for window-state
        // persistence; skip while maximized so the restored
        // position is the user's chosen spot, and while minimized:
        // Windows parks an iconic window at (-32000, -32000).
        let parked = self
            .window
            .as_ref()
            .is_some_and(|w| w.is_maximized() || w.is_minimized().unwrap_or(false));
        if !parked {
            self.win_pos = Some((pos.x, pos.y));
        }
    }

    pub(super) fn on_hovered_file(&mut self, path: &std::path::Path) {
        // A file is being dragged over the window — show the
        // full-canvas drop overlay so the target is obvious.
        if !self.host.editor_state().editor_ui.file_drop_active {
            self.host.editor_state_mut().editor_ui.file_drop_active = true;
            self.host.mark_editor_state_dirty();
            self.request_redraw(true);
        }
        // An image can land INSIDE a node, so from here on the drag
        // position is polled every frame to ring the node under it
        // (`new_events`); winit reports no cursor moves during a drag.
        if image_drop_host::is_supported_image_drop(path) {
            self.hovered_image_drop = true;
            if self.refresh_image_drop_hover() {
                // Repaint only: the ring lives in `editor_ui`, so
                // marking the document dirty here would rebuild the
                // whole layout scene on every pointer move.
                self.request_redraw(true);
            }
        }
    }

    pub(super) fn on_hovered_file_cancelled(&mut self) {
        // The drag left the window without dropping — hide it.
        self.clear_image_drop_hover();
        if self.host.editor_state().editor_ui.file_drop_active {
            self.host.editor_state_mut().editor_ui.file_drop_active = false;
            self.host.mark_editor_state_dirty();
            self.request_redraw(true);
        }
    }

    pub(super) fn on_dropped_file(&mut self, path: PathBuf) {
        // Resolve the release position BEFORE tearing down the hover
        // state — it is what decides fill-a-node vs insert-a-node.
        let drop_point = self
            .window
            .as_ref()
            .and_then(crate::drag_cursor::window_cursor_position)
            .or(self.drop_cursor);
        self.clear_image_drop_hover();
        // Clear the drag overlay now that the drop has landed.
        self.host.editor_state_mut().editor_ui.file_drop_active = false;
        self.host.mark_editor_state_dirty();
        self.request_redraw(true);
        if image_drop_host::is_supported_image_drop(&path) {
            let outcome = image_drop_host::apply_image_drop(
                &mut self.host,
                &path,
                drop_point,
                self.viewport_width,
                self.viewport_height,
            );
            if outcome == image_drop_host::ImageDropOutcome::Ignored {
                eprintln!(
                    "design-editor: dropped image had no effect: {}",
                    path.display()
                );
            }
            self.request_redraw(true);
            return;
        }
        // Drag-and-drop open. `.de` / `.pen` documents route
        // through the canonical loader; `.fig` Figma exports
        // route through the background Figma import worker
        // (the parse + layout pass takes seconds for large
        // dashboards, so doing it inline would freeze the
        // window). Anything else is ignored silently so a
        // stray drop can't disrupt the current document.
        if de_host_services::doc_io::is_supported_figma_import(&path) {
            let _ = self.begin_figma_import(path);
        } else if de_host_services::doc_io::is_supported_html_import(&path) {
            let _ = self.begin_html_import(path);
        } else if de_host_services::doc_io::is_supported_document(&path) {
            if persistence::open_path(
                &mut self.host,
                path,
                &mut self.current_path,
                self.window.as_ref(),
            ) {
                self.mark_document_saved();
                self.request_redraw(true);
            }
        } else {
            eprintln!(
                "design-editor: ignored dropped file (not .de / .pen / .fig / .html / .zip): {}",
                path.display()
            );
        }
    }

    pub(super) fn on_scale_factor_changed(&mut self, scale_factor: f64) {
        self.dpi = scale_factor as f32;
        if let Some(backend) = self.backend.as_mut() {
            backend.set_dpi(scale_factor as f32);
        }
        // Logical = physical / dpi; refresh to keep input + paint coords in sync after a DPI flip.
        // A minimized window's inner size is 0×0; keep the last real
        // viewport until the restore's `Resized` (#302).
        if let Some(size) = self
            .window
            .as_ref()
            .map(Window::inner_size)
            .filter(|size| has_area(*size))
        {
            self.viewport_width = size.width as f32 / self.dpi;
            self.viewport_height = size.height as f32 / self.dpi;
        }
        self.request_redraw(true);
    }
}

/// A client area that can be painted. Windows reports 0×0 while minimized.
fn has_area(size: PhysicalSize<u32>) -> bool {
    size.width > 0 && size.height > 0
}
