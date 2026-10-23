//! Hit-testing + hover / geometry queries for [`AgentSettingsPanel`] —
//! `hit_test` (the tab-dispatching click router) plus the per-surface
//! `*_at` hover probes and content-height maths. Carved off
//! `agent_settings_panel.rs` to keep every file under the 800-line cap.

use super::*;

/// The Agents body under the tab intro.
pub(super) fn agents_body(content: Rect) -> Rect {
    Rect::xywh(
        content.origin.x,
        content.origin.y + AGENTS_HERO_HEIGHT,
        content.size.x,
        (content.size.y - AGENTS_HERO_HEIGHT).max(0.0),
    )
}

/// The Agents body width the content-height walk assumes: the detail
/// pane's field heights do not depend on width, only the list column
/// is fixed, so any full-size width gives the same height.
const AGENTS_BODY_REFERENCE_W: f32 = 704.0;
use crate::widgets::agent_settings_panel_geometry::SECONDARY_HEADING_HAS_DESC;
use de_editor_core::agent_settings::SettingsFocus;

/// The desktop card's top and bottom padding around a secondary body.
fn secondary_card_pad() -> f32 {
    2.0 * crate::widgets::agent_settings_panel_geometry::SECONDARY_CARD_PAD_Y
}

fn secondary_hero_height() -> f32 {
    crate::widgets::agent_settings_rows::tab_intro_height(SECONDARY_HEADING_HAS_DESC)
}

impl AgentSettingsPanel<'_> {
    /// Unscrolled document-space rect for the active editable field.
    /// Hosts subtract [`Self::effective_scroll`] before comparing it with the
    /// visible content viewport.
    pub fn focused_input_rect(&self, panel: Rect) -> Option<Rect> {
        if self.active_tab() == AgentSettingsTab::Agents {
            let body = agents_body(self.resolved_content_viewport(panel));
            return self.agents_tab().focused_input_rect(body);
        }
        crate::widgets::agent_settings_focus_geometry::focused_input_rect(
            self.resolved_content_viewport(panel),
            &self.settings,
            self.active_tab(),
        )
    }

    /// Resolve a click/tap in the currently focused multiline provider Model
    /// field to its UTF-8 byte offset. The screen-space input rect applies the
    /// same outer body scroll as paint and hit-testing.
    pub fn focused_model_text_byte_offset_at(&self, panel: Rect, point: Point2D) -> Option<usize> {
        if !matches!(
            self.settings.focus,
            Some(SettingsFocus::BuiltinAgent {
                field: BuiltinAgentField::Model,
                ..
            }) | Some(SettingsFocus::BuiltinAgentDraft(BuiltinAgentField::Model))
        ) {
            return None;
        }
        let mut input = self.focused_input_rect(panel)?;
        input.origin.y -= self.effective_scroll(panel);
        crate::widgets::agent_settings_caret::model_text_byte_offset_at(self.ui, input, point)
    }

    /// Resolve a click/tap in the currently focused settings text input —
    /// multiline provider Model editor or single-line field — to its UTF-8
    /// byte offset. `None` when no focused input geometry lies under
    /// `point` (the hosts then keep the seeded caret-at-end behavior).
    pub fn focused_text_byte_offset_at(&self, panel: Rect, point: Point2D) -> Option<usize> {
        if let Some(offset) = self.focused_model_text_byte_offset_at(panel, point) {
            return Some(offset);
        }
        let focus = self.settings.focus?;
        let mut input = self.focused_input_rect(panel)?;
        input.origin.y -= self.effective_scroll(panel);
        crate::widgets::agent_settings_caret::single_line_text_byte_offset_at(
            self.ui, focus, input, point,
        )
    }

    pub fn hit_test(&self, panel: Rect, point: Point2D) -> AgentSettingsHit {
        if !panel.contains(point) {
            return AgentSettingsHit::Outside;
        }
        let layout = self.resolved_layout(panel);
        if layout.close_target.contains(point) {
            return AgentSettingsHit::Close;
        }
        let tabs = self.mode.visible_tabs();
        for (i, tab) in tabs.iter().enumerate() {
            if layout.nav_item_rect(i, tabs.len()).contains(point) {
                return AgentSettingsHit::SelectTab(*tab);
            }
        }
        let active_tab = self.mode.active_tab(&self.settings);
        let scroll = self.effective_scroll(panel);
        // The Fonts popup deliberately escapes the body clip and is painted
        // as a modal child. Route it before the body gate; close and tab
        // controls above already retained priority.
        if active_tab == AgentSettingsTab::Fonts && self.font_picker_layout(panel).is_some() {
            let hit = agent_settings_fonts::hit_test(
                panel,
                hero_body_rect_for_ui(layout.content),
                self.ui,
                point,
                scroll,
            );
            if hit != FontsHit::None {
                return AgentSettingsHit::Fonts(hit);
            }
        }
        // Painted tab bodies are clipped to this viewport. Gate dispatch by
        // the same rect so a scrolled-off control cannot fire through the
        // touch header or navigation strip.
        if !layout.content.contains(point) {
            return AgentSettingsHit::Inside;
        }
        let scrolled = Point2D::new(point.x, point.y + scroll);
        let content = layout.content;
        match active_tab {
            AgentSettingsTab::Agents => {
                return self.agents_tab().hit(agents_body(content), scrolled);
            }
            AgentSettingsTab::Mcp => {
                match agent_settings_mcp::hit_test(
                    content,
                    &self.settings,
                    self.ui.effective_locale(),
                    scrolled,
                ) {
                    McpHit::ToggleServer => return AgentSettingsHit::ToggleMcpServer,
                    McpHit::ToggleCli(cli) => return AgentSettingsHit::ToggleMcpCli(cli),
                    McpHit::CopyClientConfig => return AgentSettingsHit::CopyMcpClientConfig,
                    McpHit::FocusPort => return AgentSettingsHit::FocusMcpPort,
                    McpHit::ExpandCli(cli) => return AgentSettingsHit::ExpandMcpCli(cli),
                    McpHit::CopySnippet(cli) => return AgentSettingsHit::CopyMcpSnippet(cli),
                    McpHit::ToggleShowAll => return AgentSettingsHit::ToggleMcpShowAll,
                    McpHit::None => {}
                }
            }
            AgentSettingsTab::Images => {
                match agent_settings_images::hit_test(
                    hero_body_rect_for_ui(content),
                    &self.settings,
                    scrolled,
                ) {
                    ImagesHit::ToggleAdvanced => return AgentSettingsHit::ToggleImagesAdvanced,
                    ImagesHit::FocusSearchField(field) => {
                        return AgentSettingsHit::FocusSearchField(field);
                    }
                    ImagesHit::OpenRegisterLink => {
                        return AgentSettingsHit::OpenImageRegisterLink;
                    }
                    ImagesHit::TestSearch => return AgentSettingsHit::TestImageSearch,
                    ImagesHit::AddGenConfig => return AgentSettingsHit::AddGenConfig,
                    ImagesHit::ToggleGenConfigEditor(index) => {
                        return AgentSettingsHit::ToggleGenConfigEditor(index);
                    }
                    ImagesHit::SetActiveGenConfig(index) => {
                        return AgentSettingsHit::SetActiveGenConfig(index);
                    }
                    ImagesHit::RemoveGenConfig(index) => {
                        return AgentSettingsHit::RemoveGenConfig(index);
                    }
                    ImagesHit::TestGenConfig(index) => {
                        return AgentSettingsHit::TestGenConfig(index);
                    }
                    ImagesHit::ToggleGenProviderMenu(index) => {
                        return AgentSettingsHit::ToggleGenProviderMenu(index);
                    }
                    ImagesHit::SelectGenProvider { index, provider } => {
                        return AgentSettingsHit::SelectGenProvider { index, provider };
                    }
                    ImagesHit::FocusGenConfig { index, field } => {
                        return AgentSettingsHit::FocusGenConfig { index, field };
                    }
                    ImagesHit::None => {}
                }
            }
            AgentSettingsTab::Fonts => {
                let hit = agent_settings_fonts::hit_test(
                    panel,
                    hero_body_rect_for_ui(content),
                    self.ui,
                    point,
                    scroll,
                );
                if hit != FontsHit::None {
                    return AgentSettingsHit::Fonts(hit);
                }
            }
            AgentSettingsTab::System => {
                match agent_settings_system::hit_test(content, self.ui, scrolled) {
                    SystemHit::SelectThemeMode(mode) => {
                        return AgentSettingsHit::SelectThemeMode(mode)
                    }
                    SystemHit::ToggleAutoUpdate => return AgentSettingsHit::ToggleAutoUpdate,
                    SystemHit::UpdateButton => return AgentSettingsHit::UpdateButton,
                    SystemHit::ToggleExperimental => return AgentSettingsHit::ToggleExperimental,
                    SystemHit::SelectPencilCursor(style) => {
                        return AgentSettingsHit::SelectPencilCursor(style)
                    }
                    SystemHit::None => {}
                }
            }
        }
        AgentSettingsHit::Inside
    }

    /// A screen point inside the visible body where a press resolves to
    /// `hit`, at the current scroll — `None` when nothing visible does.
    /// Lets host tests press a control by what it does instead of copying
    /// the geometry that places it. It probes the body on a 4 px grid, so
    /// it is for tests and tooling, never a per-frame path; it stays `pub`
    /// because the host crates' tests cannot see a `cfg(test)` item here.
    #[doc(hidden)]
    pub fn point_for_hit(&self, panel: Rect, hit: AgentSettingsHit) -> Option<Point2D> {
        const STEP: f32 = 4.0;
        let content = self.resolved_content_viewport(panel);
        let mut y = content.origin.y + STEP / 2.0;
        while y < content.origin.y + content.size.y {
            let mut x = content.origin.x + STEP / 2.0;
            while x < content.origin.x + content.size.x {
                let point = Point2D::new(x, y);
                if self.hit_test(panel, point) == hit {
                    return Some(point);
                }
                x += STEP;
            }
            y += STEP;
        }
        None
    }

    pub fn nav_at(&self, panel: Rect, point: Point2D) -> Option<AgentSettingsTab> {
        let layout = self.resolved_layout(panel);
        let tabs = self.mode.visible_tabs();
        for (i, tab) in tabs.iter().enumerate() {
            if layout.nav_item_rect(i, tabs.len()).contains(point) {
                return Some(*tab);
            }
        }
        None
    }

    /// Agents tab: the scrolled point when `point` is inside the body,
    /// and the tab is the Agents one.
    fn agents_point(&self, panel: Rect, point: Point2D) -> Option<Point2D> {
        let content = self.resolved_content_viewport(panel);
        (content.contains(point) && self.active_tab() == AgentSettingsTab::Agents)
            .then(|| Point2D::new(point.x, point.y + self.effective_scroll(panel)))
    }

    /// Agents tab: the hovered list line's index (see `hover_agents_row`).
    pub fn agents_row_at(&self, panel: Rect, point: Point2D) -> Option<usize> {
        let scrolled = self.agents_point(panel, point)?;
        let body = agents_body(self.resolved_content_viewport(panel));
        self.agents_tab().row_at(body, scrolled)
    }

    /// Agents tab: the hovered row of the draft's provider dropdown.
    pub fn builtin_preset_hover_at(
        &self,
        panel: Rect,
        point: Point2D,
    ) -> Option<BuiltinAgentPresetKey> {
        let scrolled = self.agents_point(panel, point)?;
        let body = agents_body(self.resolved_content_viewport(panel));
        self.agents_tab().preset_hover_at(body, scrolled)
    }

    /// Wheel-scroll cap for the provider dropdown under `point`.
    pub fn builtin_preset_scroll_max_at(&self, panel: Rect, point: Point2D) -> Option<f32> {
        let scrolled = self.agents_point(panel, point)?;
        let body = agents_body(self.resolved_content_viewport(panel));
        self.agents_tab().preset_scroll_max_at(body, scrolled)
    }

    /// Agents tab: the hovered row of the detail's model checklist.
    pub fn agents_checklist_hover_at(&self, panel: Rect, point: Point2D) -> Option<usize> {
        let scrolled = self.agents_point(panel, point)?;
        let body = agents_body(self.resolved_content_viewport(panel));
        self.agents_tab().model_row_at(body, scrolled)
    }

    /// Wheel-scroll cap for the model checklist under `point`; `None`
    /// when the cursor is not over one.
    pub fn agents_checklist_scroll_max_at(&self, panel: Rect, point: Point2D) -> Option<f32> {
        let scrolled = self.agents_point(panel, point)?;
        let body = agents_body(self.resolved_content_viewport(panel));
        self.agents_tab().model_scroll_max_at(body, scrolled)
    }

    pub fn image_search_test_button_hover_at(&self, panel: Rect, point: Point2D) -> bool {
        let content = self.resolved_content_viewport(panel);
        if !content.contains(point) {
            return false;
        }
        let scrolled = Point2D::new(point.x, point.y + self.effective_scroll(panel));
        agent_settings_images::search_test_button_hover_at_for_ui(
            hero_body_rect_for_ui(content),
            &self.settings,
            scrolled,
        )
    }

    pub fn image_gen_add_button_hover_at(&self, panel: Rect, point: Point2D) -> bool {
        let content = self.resolved_content_viewport(panel);
        if !content.contains(point) {
            return false;
        }
        let scrolled = Point2D::new(point.x, point.y + self.effective_scroll(panel));
        agent_settings_images::add_gen_button_hover_at_for_ui(
            hero_body_rect_for_ui(content),
            &self.settings,
            scrolled,
        )
    }

    pub fn image_gen_profile_test_button_hover_at(
        &self,
        panel: Rect,
        point: Point2D,
    ) -> Option<usize> {
        let content = self.resolved_content_viewport(panel);
        if !content.contains(point) {
            return None;
        }
        let scrolled = Point2D::new(point.x, point.y + self.effective_scroll(panel));
        agent_settings_images::profile_test_button_hover_at_for_ui(
            hero_body_rect_for_ui(content),
            &self.settings,
            scrolled,
        )
    }

    pub fn content_total_height(&self) -> f32 {
        match self.mode.active_tab(&self.settings) {
            AgentSettingsTab::Agents => {
                let tab = self.agents_tab();
                // Only the body's width matters for the walk.
                let body = Rect::xywh(0.0, 0.0, AGENTS_BODY_REFERENCE_W, 0.0);
                AGENTS_HERO_HEIGHT + tab.body_height(body) + CONTENT_TAIL_PAD
            }
            AgentSettingsTab::Mcp => agent_settings_mcp::content_height(&self.settings),
            AgentSettingsTab::Images => {
                secondary_hero_height()
                    + secondary_card_pad()
                    + agent_settings_images::responsive_content_height(&self.settings)
            }
            AgentSettingsTab::Fonts => {
                secondary_hero_height()
                    + secondary_card_pad()
                    + agent_settings_fonts::content_height(self.ui)
            }
            AgentSettingsTab::System => agent_settings_system::content_height(),
        }
    }

    pub fn font_picker_layout(
        &self,
        panel: Rect,
    ) -> Option<crate::widgets::property_panel_typography::FontPickerLayout> {
        agent_settings_fonts::picker_layout(
            panel,
            hero_body_rect_for_ui(self.resolved_content_viewport(panel)),
            self.ui,
            self.effective_scroll(panel),
        )
    }
}
