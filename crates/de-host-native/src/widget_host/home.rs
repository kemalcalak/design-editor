//! Home surface input routing for the native widget host: hit-test the
//! surface, then map each target onto the existing editor action.

use super::WidgetHostNative;
use de_editor_core::home_mcp_setup::home_mcp_manual_snippet;
use de_editor_core::{AgentSettingsTab, FileAction, HomeCopyTarget, HomeHit, HomeTemplate};
use de_editor_ui::widgets::HomeSurface;
use de_editor_ui::Point2D;

impl WidgetHostNative {
    pub fn home_visible(&self) -> bool {
        self.editor_state.editor_ui.home.visible
    }

    pub(in crate::widget_host) fn press_home(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        if !self.home_visible() {
            return None;
        }
        let point = Point2D::new(x, y);
        let home = HomeSurface::for_editor_at(&self.editor_state, self.now_ms)?;
        let on_popover = home.popover_contains(viewport_width, viewport_height, point);
        let hit = home.hit_test(viewport_width, viewport_height, point);
        drop(home);
        let state = &mut self.editor_state.editor_ui.home;
        // A press outside the open popovers only closes them — including
        // a press on the control that opened one, which reads as a toggle.
        if (state.size_menu_open || state.mcp_popover_open) && !on_popover {
            state.close_popovers();
            self.mark_dirty();
            return Some(true);
        }
        let Some(hit) = hit else {
            return Some(true);
        };
        if state.mcp_agent_menu_open && !matches!(hit, HomeHit::McpAgent(_) | HomeHit::McpAgentMore)
        {
            state.mcp_agent_menu_open = false;
        }
        state.pressed = Some(hit);
        self.apply_home_hit(hit);
        self.mark_dirty();
        Some(true)
    }

    /// Run the editor action behind one Home target.
    fn apply_home_hit(&mut self, hit: HomeHit) {
        let now_ms = self.now_ms;
        let ui = &mut self.editor_state.editor_ui;
        match hit {
            HomeHit::Sheet => {
                let caret = ui.home.input.text().len();
                ui.home.set_caret(caret, now_ms);
            }
            HomeHit::Kind(family) => {
                ui.home.set_task(family);
            }
            HomeHit::SizeChip => {
                ui.home.size_menu_open = !ui.home.size_menu_open;
            }
            HomeHit::SizeOption(size) => ui.home.set_size(size),
            HomeHit::SizeMenuInside | HomeHit::McpInside => {}
            HomeHit::Attach => {
                // The chat attachment picker is the one image-input path.
                // It opens from the desktop event drain; the staged list it
                // produces is the composer's source of truth.
                self.editor_state.chat.pending_attachment_pick = true;
            }
            HomeHit::ImportScreenshot => {
                // Pick the screenshot and say what to do with it; a brief
                // the user already typed is left alone.
                if ui.home.draft.trim().is_empty() {
                    let prompt =
                        de_i18n::translate(ui.effective_locale(), "home.import.screenshotPrompt");
                    ui.home.set_draft(prompt);
                }
                self.editor_state.chat.pending_attachment_pick = true;
            }
            HomeHit::ModelChip => {
                // Same open path the chat panel's model pill takes.
                if ui.toggle_chat_model_picker() {
                    self.editor_state.rebuild_chat_models();
                    let ui = &mut self.editor_state.editor_ui;
                    ui.close_parallel_agents_picker();
                    ui.chat_model_picker_input.touch(now_ms);
                }
            }
            HomeHit::AddProvider => open_settings(ui, Some(AgentSettingsTab::Agents)),
            // Without a provider the button is greyed out and does nothing;
            // the "Add a model provider" link beside it is the way forward.
            HomeHit::Send => {
                self.queue_home_send();
            }
            HomeHit::ImportFigma => ui.pending_file_action = Some(FileAction::ImportFigma),
            HomeHit::ImportWeb => ui.pending_file_action = Some(FileAction::ImportHtml),
            HomeHit::ImportPenDev => ui.pending_file_action = Some(FileAction::ImportPenDev),
            HomeHit::Recent(index) => {
                if index < ui.recent_files.len() {
                    ui.pending_file_action = Some(FileAction::OpenRecent(index));
                }
            }
            HomeHit::RecentView(view) => ui.home.recent_view = view,
            HomeHit::NavRecents => ui.home.scroll_y = 0.0,
            HomeHit::NavOpenFile | HomeHit::OpenFile => {
                ui.pending_file_action = Some(FileAction::Open);
            }
            HomeHit::NewFile => ui.pending_file_action = Some(FileAction::New),
            HomeHit::Locale => de_editor_core::host_press_transitions::toggle_locale_picker(ui),
            HomeHit::NavDesignSystems | HomeHit::NavSceneTemplates => {
                // A template opens on a fresh document: the host's New
                // handler installs it, then applies the parked template.
                ui.home.pending_template = Some(if hit == HomeHit::NavDesignSystems {
                    HomeTemplate::DesignSystems
                } else {
                    HomeTemplate::SceneTemplates
                });
                ui.pending_file_action = Some(FileAction::New);
            }
            HomeHit::NavSettings => open_settings(ui, None),
            HomeHit::McpCard => {
                ui.home.mcp_popover_open = !ui.home.mcp_popover_open;
                ui.home.mcp_agent_menu_open = false;
            }
            HomeHit::McpCopyEndpoint => {
                let url = ui.agent_settings.mcp_endpoint_url();
                ui.home.copied = Some((HomeCopyTarget::Endpoint, now_ms.max(1)));
                self.editor_state.chat.queue_copy_text(url);
            }
            HomeHit::McpCopySnippet => {
                let snippet = home_mcp_manual_snippet(
                    ui.home.mcp_agent,
                    &ui.agent_settings.mcp_endpoint_url(),
                );
                ui.home.copied = Some((HomeCopyTarget::Snippet, now_ms.max(1)));
                self.editor_state.chat.queue_copy_text(snippet);
            }
            HomeHit::McpAgent(cli) => {
                ui.home.mcp_agent = cli;
                ui.home.mcp_agent_menu_open = false;
            }
            HomeHit::McpAgentMore => {
                ui.home.mcp_agent_menu_open = !ui.home.mcp_agent_menu_open;
            }
            HomeHit::McpSetUp | HomeHit::McpRemove => {
                // The same slot the Settings ▸ MCP toggle flips; the desktop
                // reconciles the agent's config file after this event.
                ui.agent_settings
                    .set_mcp_cli_enabled(ui.home.mcp_agent, hit == HomeHit::McpSetUp);
            }
            HomeHit::McpStart => ui.agent_settings.mcp_server.running = true,
            HomeHit::McpSettings => {
                ui.home.close_popovers();
                open_settings(ui, Some(AgentSettingsTab::Mcp));
            }
        }
    }

    pub(in crate::widget_host) fn cursor_move_home(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<bool> {
        if !self.home_visible() {
            return None;
        }
        // A dialog Home opened (Settings, export, …) owns the pointer: step
        // aside so the modal tier below takes the move, and drop whatever
        // Home had lit up when the dialog opened (#86).
        if self.editor_state.editor_ui.pointer_modal_open() {
            if self.editor_state.editor_ui.clear_hover_behind_modal() {
                self.mark_dirty();
            }
            return None;
        }
        let point = Point2D::new(x, y);
        let home = HomeSurface::for_editor_at(&self.editor_state, self.now_ms)?;
        let next = home.hit_test(viewport_width, viewport_height, point);
        drop(home);
        // Home paints the window-control dots in the same strip as the
        // editor's left rail; hovering them reveals their glyphs there too.
        let traffic = de_editor_ui::widgets::window_strip::window_controls_rect().contains(point);
        let ui = &mut self.editor_state.editor_ui;
        if ui.home.hover == next && ui.window_control_hover == traffic {
            return Some(true);
        }
        ui.home.hover = next;
        ui.window_control_hover = traffic;
        self.mark_dirty();
        Some(true)
    }

    /// Whether a press at `(x, y)` should drag the borderless window:
    /// Home's title strip outside every Home control and popover. While
    /// an overlay Home opened is up (Settings, the language menu,
    /// the model picker, which can reach the strip) the press is theirs.
    pub fn home_window_drag_at(
        &self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let ui = &self.editor_state.editor_ui;
        if ui.agent_settings_open || ui.locale_picker.open || ui.chat_model_picker.open {
            return false;
        }
        HomeSurface::for_editor_at(&self.editor_state, self.now_ms).is_some_and(|home| {
            home.window_drag_at(viewport_width, viewport_height, Point2D::new(x, y))
        })
    }

    /// Whether a press at `(x, y)` on the editor drags the window: the left
    /// rail's window strip, or the canvas's top band where nothing sits —
    /// no floating chrome, no open dropdown or panel, and no canvas node
    /// (design content reaching the top of the canvas keeps its clicks).
    pub fn editor_window_drag_at(
        &self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let point = Point2D::new(x, y);
        if !de_editor_ui::widgets::host_canvas_geometry::window_drag_band_at(
            &self.editor_state,
            viewport_width,
            viewport_height,
            point,
        ) {
            return false;
        }
        if self.over_dropdown_overlay(x, y, viewport_width, viewport_height)
            || self.over_topmost_panel(x, y, viewport_width, viewport_height)
            // Preview's device pill floats inside the strip, and window drag
            // is decided before `apply_press` runs, so the pill opts out here.
            || self.over_preview_switcher(x, y, viewport_width, viewport_height)
        {
            return false;
        }
        // A selection's handles reach a few px outside its node, so a node
        // sitting just under the band still owns its handle presses.
        let (cx, cy, cw, ch) = de_editor_ui::widgets::host_canvas_geometry::canvas_region(
            &self.editor_state,
            viewport_width,
            viewport_height,
        );
        if de_editor_ui::widgets::selection_handle_at_point(
            de_editor_ui::Rect::xywh(cx, cy, cw, ch),
            &self.layout_scene,
            &self.editor_state,
            point,
        )
        .is_some()
        {
            return false;
        }
        match de_editor_ui::widgets::host_canvas_geometry::canvas_doc_point(
            &self.editor_state,
            x,
            y,
            viewport_width,
            viewport_height,
        ) {
            Some(doc_point) => self
                .layout_scene
                .node_path_at_doc_point(doc_point, self.editor_state.viewport.zoom)
                .is_none(),
            // The rail's strip — off the canvas, nothing to hit there.
            None => true,
        }
    }

    /// Queue Home's brief as a generation turn. Refused (returns false)
    /// while no agent can answer it, the same rule that greys Send out.
    pub(in crate::widget_host) fn queue_home_send(&mut self) -> bool {
        if !self.editor_state.has_usable_chat_agent() {
            return false;
        }
        let Some(prompt) = self.editor_state.editor_ui.home.generation_prompt() else {
            return false;
        };
        // A brief started from Home is a NEW deliverable, so it needs a
        // page of its own. Without this the run appends to whatever the
        // last one drew and the two designs share a canvas, a deck strip
        // and a transcript. A page that is still the untouched starter has
        // nothing to protect, so the very first run keeps the document it
        // was launched on.
        if !de_editor_core::blank_starter::active_page_is_blank_starter(&self.editor_state) {
            self.start_fresh_document_for_home();
        }
        // Open the generation workspace on the SAME document: the chat
        // pins into the dock and the canvas renders the boards the run
        // produces. Kind, brief, and size are captured now because Home is
        // about to hide. `run_epoch = 0` until the desktop launch stamps
        // the live agent epoch.
        let family = self.editor_state.editor_ui.home.task;
        let brief = self.editor_state.editor_ui.home.draft.trim().to_string();
        let options = self.editor_state.editor_ui.home.task_draft().clone();
        let previous_tool = Some(self.editor_state.tool);
        // The workspace opener also opens the LEFT PANEL on the Chat tab
        // (the one-time width bump included) — the dock is the rail, so a
        // run always lands with its conversation visible.
        self.editor_state.editor_ui.open_workspace_for_generation(
            family,
            brief,
            options,
            0,
            self.now_ms,
            previous_tool,
        );
        // Pan-only viewing while the workspace owns the canvas; the
        // previous tool is restored on Professional Editing.
        self.editor_state.tool = de_editor_core::Tool::Hand;
        self.editor_state.editor_ui.home.hide();
        self.editor_state.chat.focus_input_at_end(self.now_ms);
        self.editor_state.chat.set_input_text(prompt);
        // Home briefs are whole-design requests — pin the turn to the
        // orchestrator pipeline. The desktop launcher consumes the route
        // on the next drain.
        self.editor_state.chat.launch_route = de_editor_core::LaunchRoute::Orchestrator;
        let sent = self.editor_state.chat.begin_send();
        self.editor_state.chat.focused = false;
        sent
    }

    pub(in crate::widget_host) fn home_text(&mut self, c: char) -> bool {
        if !self.home_visible() || c.is_control() {
            return false;
        }
        let changed = self
            .editor_state
            .editor_ui
            .home
            .insert_text(&c.to_string(), self.now_ms);
        if changed {
            self.mark_dirty();
        }
        true
    }

    pub(in crate::widget_host) fn home_backspace(&mut self) -> Option<bool> {
        if !self.home_visible() {
            return None;
        }
        self.editor_state.editor_ui.home.backspace(self.now_ms);
        self.mark_dirty();
        Some(true)
    }

    pub(in crate::widget_host) fn home_delete(&mut self) -> Option<bool> {
        if !self.home_visible() {
            return None;
        }
        self.editor_state.editor_ui.home.delete_forward(self.now_ms);
        self.mark_dirty();
        Some(true)
    }

    pub(in crate::widget_host) fn home_caret(&mut self, forward: bool, extend: bool) -> bool {
        if !self.home_visible() {
            return false;
        }
        self.editor_state
            .editor_ui
            .home
            .move_caret(forward, extend, self.now_ms);
        self.mark_dirty();
        true
    }

    pub(in crate::widget_host) fn home_select_all(&mut self) -> bool {
        if !self.home_visible() {
            return false;
        }
        self.editor_state.editor_ui.home.select_all(self.now_ms);
        self.mark_dirty();
        true
    }

    pub(in crate::widget_host) fn home_ime_preedit(
        &mut self,
        text: &str,
        cursor: Option<(usize, usize)>,
    ) -> bool {
        if !self.home_visible() {
            return false;
        }
        let input = &mut self.editor_state.editor_ui.home.input;
        if text.is_empty() {
            input.clear_composition();
        } else {
            let (start, end) = cursor.unwrap_or((text.len(), text.len()));
            input.set_composing_text(text, start, end, self.now_ms);
        }
        self.mark_dirty();
        true
    }

    pub(in crate::widget_host) fn home_ime_commit(&mut self, text: &str) -> bool {
        if !self.home_visible() {
            return false;
        }
        if !text.is_empty() {
            let home = &mut self.editor_state.editor_ui.home;
            home.input.commit_text(text, self.now_ms);
            home.sync_committed_input();
        }
        self.mark_dirty();
        true
    }

    /// Escape's Home rung: popovers, then Home itself when it was opened
    /// over a document (a running workspace takes the view back).
    pub(in crate::widget_host) fn escape_home(&mut self) -> bool {
        let ui = &mut self.editor_state.editor_ui;
        if !ui.home.visible || !ui.home.escape() {
            return false;
        }
        if !ui.home.visible && ui.workspace.active {
            ui.workspace.reenter(self.now_ms);
        }
        self.mark_dirty();
        true
    }
}

/// Open the Settings modal, optionally on one tab.
fn open_settings(ui: &mut de_editor_core::EditorUiState, tab: Option<AgentSettingsTab>) {
    ui.agent_settings_open = true;
    if let Some(tab) = tab {
        ui.agent_settings.tab = tab;
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
