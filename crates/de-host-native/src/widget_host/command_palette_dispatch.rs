//! Native arm over the shared command palette / find layers (#395).
//!
//! The palette's state and transitions live in
//! `de_editor_core::command_palette`, its hit-tests and camera move in
//! `de_editor_ui::widgets::command_palette_flow`; this file supplies the
//! viewport, the clock, the scene and the `mark_dirty` tails, maps each
//! [`PaletteCommand`] onto the call its shortcut makes, and paints the
//! modal with its scrim. Commands only the desktop runner can carry out
//! are queued on `editor_ui.palette_runner_request`.

use super::WidgetHostNative;
use crate::backend::NativeFrameBackend;
use de_editor_core::command_palette::{
    self as palette, PaletteCommand, PaletteMode, PaletteOutcome, PaletteRunnerRequest,
};
use de_editor_core::path_ops::PathOperation;
use de_editor_core::{BooleanOp, FlipAxis, ReorderDirection, Tool};
use de_editor_ui::widgets::command_palette_flow as flow;
use de_editor_ui::widgets::{CommandPalettePanel, PaintCx};
use de_editor_ui::{Point2D, Rect, RenderBackend};

impl WidgetHostNative {
    /// Whether the palette is open.
    pub fn command_palette_open(&self) -> bool {
        self.editor_state.command_palette_open()
    }

    /// A modal dialog (Rename layers, the command palette) is open over the
    /// document, so no chord may edit it.
    pub fn modal_dialog_open(&self) -> bool {
        self.rename_layers_open() || self.command_palette_open()
    }

    /// Cmd+K / Cmd+/ (commands) and Cmd+F (find layers): open the palette,
    /// close it with its own chord, switch with the other. Not on Home, in
    /// Preview, or over the Rename-layers preview.
    pub fn apply_toggle_command_palette(&mut self, mode: PaletteMode) -> bool {
        if self.preview.is_some() || self.editor_state.editor_ui.home.visible {
            return false;
        }
        // Nor over another modal (Rename layers, Settings, the Prompt Center).
        let ui = &self.editor_state.editor_ui;
        if self.rename_layers_open() || ui.agent_settings_open || ui.prompt_center.open {
            return false;
        }
        if !self.command_palette_open() {
            // Land a half-typed field before the palette takes the keyboard.
            self.commit_pending_input_pub();
            self.close_image_popovers_for_higher_overlay();
        }
        self.editor_state.toggle_command_palette(mode, self.now_ms);
        self.mark_dirty();
        true
    }

    /// The runner's queued request, if any (file dialogs, Git, chat tabs).
    pub fn take_palette_runner_request(&mut self) -> Option<PaletteRunnerRequest> {
        self.editor_state.editor_ui.palette_runner_request.take()
    }

    /// A typed character while the palette is open.
    pub fn palette_text(&mut self, c: char) -> bool {
        self.palette_key(|state, now| palette::command_palette_text(state, c, now))
    }

    pub fn palette_backspace(&mut self) -> bool {
        self.palette_key(palette::command_palette_backspace)
    }

    pub fn palette_caret(&mut self, forward: bool, select: bool) -> bool {
        self.palette_key(|state, now| palette::command_palette_caret(state, forward, select, now))
    }

    pub fn palette_move(&mut self, delta: i32) -> bool {
        self.palette_key(|state, _| palette::command_palette_move(state, delta))
    }

    pub fn palette_tab(&mut self) -> bool {
        self.palette_key(palette::command_palette_tab)
    }

    pub fn palette_select_all(&mut self) -> bool {
        self.palette_key(|state, _| palette::command_palette_select_all(state))
    }

    pub fn palette_paste(&mut self, text: &str) -> bool {
        self.palette_key(|state, now| palette::command_palette_paste(state, text, now))
    }

    pub fn palette_escape(&mut self) -> bool {
        self.palette_key(|state, _| palette::command_palette_escape(state))
    }

    /// Enter / Shift+Enter: run the highlighted row (or replace).
    pub fn palette_activate(&mut self, all: bool) -> bool {
        let Some(outcome) = palette::command_palette_activate(&mut self.editor_state, all) else {
            return false;
        };
        self.run_palette_outcome(outcome);
        self.mark_dirty();
        true
    }

    fn palette_key(
        &mut self,
        transition: impl FnOnce(&mut de_editor_core::EditorState, u64) -> Option<bool>,
    ) -> bool {
        match transition(&mut self.editor_state, self.now_ms) {
            None => false,
            Some(changed) => {
                if changed {
                    self.mark_dirty();
                }
                true
            }
        }
    }

    /// Carry out what an activation asked for.
    pub(in crate::widget_host) fn run_palette_outcome(&mut self, outcome: PaletteOutcome) {
        let (w, h) = (self.last_viewport_w, self.last_viewport_h);
        match outcome {
            PaletteOutcome::Command(command) => {
                self.run_palette_command(command);
            }
            PaletteOutcome::OpenRecent(index) => {
                self.editor_state.editor_ui.palette_runner_request =
                    Some(PaletteRunnerRequest::OpenRecent(index));
            }
            PaletteOutcome::Frame(ids) => {
                self.mark_dirty();
                self.refresh_layout_scene();
                flow::frame_nodes(&mut self.editor_state, &self.layout_scene, &ids, w, h);
            }
            PaletteOutcome::FitPage => self.fit_active_page_after_switch(w, h),
            PaletteOutcome::OpenVariables => {
                if !self.editor_state.editor_ui.variables_panel_open {
                    self.apply_toggle_variables_panel();
                }
            }
            PaletteOutcome::Replaced(_) | PaletteOutcome::Nothing => {}
        }
    }

    /// Run a command the way its shortcut does. `false` when it did
    /// nothing (nothing selected, a refused path op, …). The one place a
    /// palette command runs: those only the desktop runner can carry out
    /// ([`PaletteCommand::needs_runner`]) are queued here for it.
    pub fn run_palette_command(&mut self, command: PaletteCommand) -> bool {
        use PaletteCommand as C;
        match command {
            C::FindLayers => self.apply_toggle_command_palette(PaletteMode::FindLayers),
            C::Undo => self.apply_undo(),
            C::Redo => self.apply_redo(),
            C::Duplicate => self.apply_duplicate(),
            C::Delete => self.apply_delete(),
            C::SelectAll => self.apply_select_all(),
            C::Group => self.apply_group(),
            C::Ungroup => self.apply_ungroup(),
            C::CreateComponent => self.apply_create_component(),
            C::RenameLayers => self.apply_open_rename_layers(),
            C::BooleanUnion => self.apply_boolean_op(BooleanOp::Union),
            C::BooleanSubtract => self.apply_boolean_op(BooleanOp::Subtract),
            C::BooleanIntersect => self.apply_boolean_op(BooleanOp::Intersect),
            C::BooleanExclude => self.apply_boolean_op(BooleanOp::Exclude),
            C::Flatten => self.apply_path_operation(PathOperation::Flatten),
            C::OutlineStroke => self.apply_path_operation(PathOperation::OutlineStroke),
            C::FlipHorizontal => self.apply_flip(FlipAxis::Horizontal),
            C::FlipVertical => self.apply_flip(FlipAxis::Vertical),
            C::BringForward => self.apply_reorder(ReorderDirection::Up),
            C::SendBackward => self.apply_reorder(ReorderDirection::Down),
            C::ToolSelect => self.palette_tool(Tool::Select),
            C::ToolFrame => self.palette_tool(Tool::Frame),
            C::ToolRectangle => self.palette_tool(Tool::Rect),
            C::ToolEllipse => self.palette_tool(Tool::Ellipse),
            C::ToolLine => self.palette_tool(Tool::Line),
            C::ToolPolygon => self.palette_tool(Tool::Polygon),
            C::ToolPen => self.palette_tool(Tool::Pen),
            C::ToolPencil => self.palette_tool(Tool::Pencil),
            C::ToolText => self.palette_tool(Tool::Text),
            C::ToolNote => self.palette_tool(Tool::Note),
            C::ToolHand => self.palette_tool(Tool::Hand),
            C::Preview => self.toggle_preview_with_cached_viewport(),
            C::Present => self.toggle_present_with_cached_viewport(),
            C::FitAll => {
                let (w, h) = (self.last_viewport_w, self.last_viewport_h);
                self.fit_content_to_viewport(w, h);
                self.mark_dirty();
                true
            }
            C::TogglePanel => self.apply_toggle_panel(),
            C::ToggleCodePanel => self.apply_toggle_code_panel(),
            C::ToggleVariables => self.apply_toggle_variables_panel(),
            C::ToggleDesignMd => self.apply_toggle_design_md_panel(),
            C::ComponentBrowser => self.apply_toggle_component_browser(),
            C::TogglePrototypeLinks => {
                let prototype = &mut self.editor_state.editor_ui.prototype;
                prototype.show_links = !prototype.show_links;
                self.mark_dirty();
                true
            }
            C::TogglePixelGridSnap => self.apply_toggle_pixel_grid_snap(),
            C::ImportFigma => self.apply_open_figma_import(),
            C::ImportHtml => self.apply_open_html_import(),
            C::Settings => self.apply_toggle_agent_settings(),
            C::NewChat
            | C::GitPanel
            | C::NewFile
            | C::OpenFile
            | C::Save
            | C::SaveAs
            | C::ExportImage => {
                debug_assert!(command.needs_runner());
                self.editor_state.editor_ui.palette_runner_request =
                    Some(PaletteRunnerRequest::Command(command));
                true
            }
        }
    }

    fn palette_tool(&mut self, tool: Tool) -> bool {
        self.apply_set_tool(tool);
        true
    }

    /// Route a press. The palette is modal: every press is consumed while
    /// it is open.
    pub(in crate::widget_host) fn dispatch_command_palette_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        let Some(outcome) = flow::press(
            &mut self.editor_state,
            viewport_width,
            viewport_height,
            Point2D::new(x, y),
            self.now_ms,
        ) else {
            return false;
        };
        self.run_palette_outcome(outcome);
        self.mark_dirty();
        true
    }

    /// Wheel: scrolls the rows, and is swallowed anywhere else while the
    /// palette is open.
    pub(in crate::widget_host) fn try_scroll_command_palette(
        &mut self,
        x: f32,
        y: f32,
        delta_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> bool {
        if !self.command_palette_open() {
            return false;
        }
        if flow::scroll(
            &mut self.editor_state,
            x,
            y,
            delta_y,
            viewport_width,
            viewport_height,
        ) == Some(true)
        {
            self.mark_dirty();
        }
        true
    }

    /// Hover tint; `None` while the palette is closed.
    pub(in crate::widget_host) fn update_command_palette_hover(
        &mut self,
        x: f32,
        y: f32,
    ) -> Option<bool> {
        if !self.command_palette_open() {
            return None;
        }
        let changed = flow::hover(
            &mut self.editor_state,
            self.last_viewport_w,
            self.last_viewport_h,
            Point2D::new(x, y),
        );
        if changed {
            self.mark_dirty();
        }
        Some(changed)
    }

    pub(in crate::widget_host) fn paint_command_palette(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        viewport_width: f32,
        viewport_height: f32,
    ) {
        let Some(panel) = CommandPalettePanel::for_editor(&self.editor_state) else {
            return;
        };
        frame.fill_rect(
            Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(viewport_width, viewport_height),
            },
            de_editor_ui::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.25,
            },
        );
        let rect = panel.rect(viewport_width, viewport_height);
        let mut cx = PaintCx {
            backend: &mut *frame,
        };
        panel.paint_at(&mut cx, rect, self.now_ms);
    }
}

#[cfg(test)]
#[path = "command_palette_dispatch_tests.rs"]
mod tests;
