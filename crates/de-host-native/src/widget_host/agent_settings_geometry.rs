//! Agent Settings geometry shared by paint and input paths.

use super::WidgetHostNative;
use de_editor_ui::widgets::agent_settings_panel::AgentSettingsPanel;
use de_editor_ui::Rect;

impl WidgetHostNative {
    /// Resolve the settings surface and its rect against the editor viewport.
    pub(in crate::widget_host) fn agent_settings_geometry(
        &self,
        viewport_w: f32,
        viewport_h: f32,
    ) -> (AgentSettingsPanel<'_>, Rect) {
        let panel = AgentSettingsPanel::for_editor_at(&self.editor_state, self.now_ms);
        let panel_rect = panel.rect(viewport_w, viewport_h);
        (panel, panel_rect)
    }
}
