//! Geometry, hit-testing and hover for the chat's dropdowns (#26): the
//! session list under the session select, the composer's "+" menu and the
//! parallel-agents picker it opens. Each is modal while open: a press on
//! a row acts, any other press closes it. Split off `ai_chat_panel_hit.rs`
//! to keep that file under the 800-line cap.

use super::ai_chat_panel::AIChatPlaceholder;
use crate::widgets::ai_chat_hit::AIChatHit;
use crate::widgets::ai_chat_panel_footer::{
    parallel_agents_picker_rect, PARALLEL_AGENTS_COUNT, PARALLEL_AGENTS_ROW_H_PUB,
};
use crate::widgets::ai_chat_panel_header::{header_layout, HeaderLayout};
use crate::widgets::ai_chat_plus_menu::{plus_menu_rect, plus_menu_row_at, plus_menu_row_hit};
use crate::widgets::ai_chat_session_menu::{session_menu_hit, session_menu_rect};
use crate::{Point2D, Rect};

impl<'a> AIChatPlaceholder<'a> {
    /// The session row's controls for the panel laid out at `rect`.
    pub(crate) fn header(&self, rect: Rect) -> HeaderLayout {
        header_layout(rect, self.column_pinned, &self.label_new)
    }

    /// The open session list, or `None` when it is closed.
    pub(crate) fn session_menu_bounds(&self, rect: Rect) -> Option<Rect> {
        (self.session_menu_open && !self.composer_only && !self.state.is_minimized())
            .then(|| session_menu_rect(self.header(rect).select, self.tabs_snapshot.len()))
    }

    /// The open "+" menu, or `None` when it is closed.
    pub(crate) fn plus_menu_bounds(&self, rect: Rect) -> Option<Rect> {
        (self.plus_menu_open && (self.composer_only || !self.state.is_minimized()))
            .then(|| plus_menu_rect(self.footer_layout(rect).plus, rect))
    }

    /// The open parallel-agents picker, or `None` when it is closed.
    pub(crate) fn parallel_picker_bounds(&self, rect: Rect) -> Option<Rect> {
        self.parallel_agents_picker_open
            .then(|| parallel_agents_picker_rect(&self.footer_layout(rect)))
    }

    fn parallel_row_at(picker: Rect, point: Point2D) -> Option<u32> {
        let rows_top = picker.origin.y + 32.0;
        (1..=PARALLEL_AGENTS_COUNT).find(|i| {
            let row_y = rows_top + (i - 1) as f32 * PARALLEL_AGENTS_ROW_H_PUB;
            point.y >= row_y && point.y < row_y + PARALLEL_AGENTS_ROW_H_PUB
        })
    }

    /// The press an open dropdown claims. A press on a row acts; any other
    /// press closes the dropdown it missed (its own toggle) — except a
    /// press on the control that opened it, which the regular header and
    /// footer hit-tests resolve to the same toggle. `None` when no
    /// dropdown is open.
    pub(crate) fn open_menu_hit(&self, rect: Rect, point: Point2D) -> Option<AIChatHit> {
        if let Some(menu) = self.session_menu_bounds(rect) {
            if menu.contains(point) {
                return Some(
                    match session_menu_hit(menu, self.tabs_snapshot.len(), point, self.tab_hover) {
                        Some((index, true)) => AIChatHit::CloseTab(index),
                        Some((index, false)) => AIChatHit::SwitchTab(index),
                        None => AIChatHit::Inside,
                    },
                );
            }
            return Some(AIChatHit::ToggleSessionMenu);
        }
        if let Some(menu) = self.plus_menu_bounds(rect) {
            if menu.contains(point) {
                return Some(match plus_menu_row_at(menu, point) {
                    Some(row) => plus_menu_row_hit(row, self.is_streaming()),
                    None => AIChatHit::Inside,
                });
            }
            return Some(AIChatHit::TogglePlusMenu);
        }
        if let Some(picker) = self.parallel_picker_bounds(rect) {
            if picker.contains(point) {
                return Some(match Self::parallel_row_at(picker, point) {
                    Some(n) => AIChatHit::SetParallelAgents(n),
                    None => AIChatHit::Inside,
                });
            }
            return Some(AIChatHit::ToggleParallelAgentsPicker);
        }
        None
    }

    /// The session-list row under the cursor (stored in
    /// `EditorUiState.chat_tab_hover`). `None` when the list is closed.
    pub fn tab_hover_at(&self, rect: Rect, point: Point2D) -> Option<usize> {
        let menu = self.session_menu_bounds(rect)?;
        session_menu_hit(menu, self.tabs_snapshot.len(), point, None).map(|(index, _)| index)
    }

    /// The "+" menu row under the cursor.
    pub fn plus_menu_hover_at(&self, rect: Rect, point: Point2D) -> Option<u8> {
        plus_menu_row_at(self.plus_menu_bounds(rect)?, point)
    }

    /// Which row (1–6) of the open Parallel Agents picker the cursor is
    /// over. `None` when the picker is closed or the cursor is outside it.
    pub fn parallel_agents_picker_hover_at(&self, rect: Rect, point: Point2D) -> Option<u32> {
        if self.state.is_minimized() && !self.composer_only {
            return None;
        }
        let picker = self.parallel_picker_bounds(rect)?;
        if !picker.contains(point) {
            return None;
        }
        Self::parallel_row_at(picker, point)
    }
}
