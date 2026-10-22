//! Shared visibility for the left-rail editor chrome.

use super::WidgetHostNative;

impl WidgetHostNative {
    /// Whether the LAYERS TREE is showing — the rail is open, no other
    /// tab owns its body, and the workspace's dock (which pins the chat
    /// over the whole column) is not up. Every layer-tree interaction —
    /// row clicks, the drag peek, the context menu, the wheel — gates on
    /// this so an invisible tree never eats input under the chat panel
    /// or the workspace header.
    pub(in crate::widget_host) fn layers_panel_visible(&self) -> bool {
        if !self.left_rail_visible() {
            return false;
        }
        if de_editor_ui::widgets::slides_panel_flow::chat_tab_active(&self.editor_state) {
            return false;
        }
        self.editor_state.editor_ui.sidebar_open
    }
}
