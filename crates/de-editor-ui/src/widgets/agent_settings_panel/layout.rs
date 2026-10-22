//! Shell geometry for Agent Settings.
//!
//! Paint, hit-testing, scrolling, and host-facing viewport queries all resolve
//! this value instead of independently rebuilding the shell layout.

use crate::widgets::agent_settings_panel_geometry::{
    close_rect, content_paint_clip_rect, content_rect, nav_item_rect,
};
use crate::{Point2D, Rect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentSettingsLayout {
    pub surface: Rect,
    pub header: Rect,
    pub navigation: Rect,
    pub content: Rect,
    pub close_target: Rect,
    pub close_icon: Rect,
}

impl AgentSettingsLayout {
    pub(super) fn resolve(panel: Rect, tab_count: usize) -> Self {
        let close = close_rect(panel);
        Self {
            surface: panel,
            header: Rect {
                origin: panel.origin,
                size: Point2D::new(super::NAV_W, super::NAV_TITLE_H),
            },
            navigation: Rect {
                origin: Point2D::new(panel.origin.x + 10.0, panel.origin.y + super::NAV_TITLE_H),
                size: Point2D::new(
                    super::NAV_W - 20.0,
                    tab_count.max(1) as f32 * (super::NAV_ITEM_H + super::NAV_ITEM_GAP),
                ),
            },
            content: content_rect(panel),
            close_target: close,
            close_icon: close,
        }
    }

    pub fn nav_item_rect(&self, index: usize, count: usize) -> Rect {
        nav_item_rect(self.surface, index, count)
    }

    pub fn content_paint_clip(&self) -> Rect {
        content_paint_clip_rect(self.surface)
    }
}
