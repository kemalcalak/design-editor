//! Floating-menu geometry + hit-testing for the `PropertyPanel` —
//! the Effects "+" add-menu and the shared Interactions / Motion
//! popover.
//!
//! Split out of `property_panel.rs` to keep both files under the
//! design-editor 800-line cap.

use super::{EffectAddMenuHit, PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_interactions::InteractionMenuHit;
use crate::widgets::property_panel_sections as sections;
use crate::{Point2D, Rect};

impl PropertyPanel {
    /// Hit-test the Effects "+" add-menu against `point` (panel space).
    /// `Row` = a choice was clicked, `Inside` = swallow (keep open),
    /// `Outside` = dismiss. Only meaningful while the menu is open.
    pub fn effect_add_menu_hit(&self, panel_rect: Rect, point: Point2D) -> EffectAddMenuHit {
        self.effect_add_menu_hit_logical(panel_rect, point)
    }

    pub(super) fn effect_add_menu_hit_logical(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> EffectAddMenuHit {
        let Some(menu) = self.effect_add_menu_rect_logical(panel_rect) else {
            return EffectAddMenuHit::Outside;
        };
        for (action, row) in crate::widgets::property_panel_effects::effect_add_menu_row_rects(menu)
        {
            if row.contains(point) {
                return EffectAddMenuHit::Row(action);
            }
        }
        if menu.contains(point) {
            EffectAddMenuHit::Inside
        } else {
            EffectAddMenuHit::Outside
        }
    }

    /// Exact painted bounds of the open Effects add-menu, including its
    /// padded chrome. Hosts use this as the O(1) ownership/occlusion test;
    /// deriving it here keeps popup paint, hit, and hover geometry identical.
    pub fn effect_add_menu_rect(&self, panel_rect: Rect) -> Option<Rect> {
        self.effect_add_menu_rect_logical(panel_rect)
    }

    fn effect_add_menu_rect_logical(&self, panel_rect: Rect) -> Option<Rect> {
        if !self.effect_add_picker_open {
            return None;
        }
        self.effect_add_button_rect(self.scrolled_rect(panel_rect))
            .map(crate::widgets::property_panel_effects::effect_add_menu_rect)
    }

    /// Whether `point` lies anywhere inside the open Effects add-menu chrome.
    pub fn effect_add_menu_contains(&self, panel_rect: Rect, point: Point2D) -> bool {
        self.effect_add_menu_rect(panel_rect)
            .is_some_and(|menu| menu.contains(point))
    }

    /// Row index under `point` in the open Effects add-menu — drives the
    /// hover highlight (mirrors [`Self::export_picker_row_at`]).
    pub fn effect_add_menu_row_at(&self, panel_rect: Rect, point: Point2D) -> Option<usize> {
        let menu = self.effect_add_menu_rect_logical(panel_rect)?;
        crate::widgets::property_panel_effects::effect_add_menu_row_rects(menu)
            .into_iter()
            .position(|(_, row)| row.contains(point))
    }

    /// The Effects section "+" button rect — `scrolled` is the already
    /// scroll-adjusted panel rect (`scrolled_rect`). The anchor the
    /// add-menu popover drops from.
    pub(crate) fn effect_add_button_rect(&self, scrolled: Rect) -> Option<Rect> {
        sections::action_button_rects_with_fill_picker(
            scrolled,
            self.visible_sections(),
            &self.snapshot.effects,
            &self.snapshot.fills,
            &self.snapshot.interactions,
            self.fill_type_picker.open,
            self.fill_type_picker_index,
            self.font_picker.open,
            self.font_weight_picker_open,
            self.export_scale_picker_open,
            self.export_format_picker_open,
            self.layout_align_popover_open,
        )
        .into_iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleEffectAddPicker))
        .map(|(_, r)| r)
    }

    /// The open popover's rows for the current target.
    pub(super) fn interaction_menu_rows(&self) -> Vec<(PropertyPanelAction, String)> {
        crate::widgets::property_panel_interactions::interaction_menu_rows(
            self.locale,
            self.interaction_menu_target,
            &self.screen_paths,
            &self.snapshot.interactions,
        )
    }

    /// Hit-test the shared Interactions / Motion popover against `point`
    /// (panel space). Mirrors [`Self::effect_add_menu_hit`].
    pub fn interaction_menu_hit(&self, panel_rect: Rect, point: Point2D) -> InteractionMenuHit {
        self.interaction_menu_hit_logical(panel_rect, point)
    }

    pub(super) fn interaction_menu_hit_logical(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> InteractionMenuHit {
        if !self.interaction_menu_open {
            return InteractionMenuHit::Outside;
        }
        let Some(anchor) = self.interaction_menu_anchor_rect(self.scrolled_rect(panel_rect)) else {
            return InteractionMenuHit::Outside;
        };
        let rows = self.interaction_menu_rows();
        let menu =
            crate::widgets::property_panel_interactions::interaction_menu_rect(anchor, rows.len());
        for (action, row) in
            crate::widgets::property_panel_interactions::interaction_menu_row_rects(menu, &rows)
        {
            if row.contains(point) {
                return InteractionMenuHit::Row(action);
            }
        }
        if menu.contains(point) {
            InteractionMenuHit::Inside
        } else {
            InteractionMenuHit::Outside
        }
    }

    /// Row index under `point` in the open popover — drives the hover
    /// highlight (mirrors [`Self::effect_add_menu_row_at`]).
    pub fn interaction_menu_row_at(&self, panel_rect: Rect, point: Point2D) -> Option<usize> {
        if !self.interaction_menu_open {
            return None;
        }
        let anchor = self.interaction_menu_anchor_rect(self.scrolled_rect(panel_rect))?;
        let rows = self.interaction_menu_rows();
        let menu =
            crate::widgets::property_panel_interactions::interaction_menu_rect(anchor, rows.len());
        crate::widgets::property_panel_interactions::interaction_menu_row_rects(menu, &rows)
            .into_iter()
            .position(|(_, row)| row.contains(point))
    }

    /// The rect the popover drops from: the row that opened it
    /// (`ToggleInteractionMenu(target)`). A trigger just picked from the
    /// "+" list has no row yet, so it stays on the header "+". `None`
    /// when neither applies (the selection or document changed under
    /// the popover), so the host's outside-click closes it.
    pub fn interaction_menu_anchor_rect(&self, scrolled: Rect) -> Option<Rect> {
        use de_editor_core::interaction_edit::InteractionMenuTarget as T;
        let rects = sections::action_button_rects_with_fill_picker(
            scrolled,
            self.visible_sections(),
            &self.snapshot.effects,
            &self.snapshot.fills,
            &self.snapshot.interactions,
            self.fill_type_picker.open,
            self.fill_type_picker_index,
            self.font_picker.open,
            self.font_weight_picker_open,
            self.export_scale_picker_open,
            self.export_format_picker_open,
            self.layout_align_popover_open,
        );
        let find = |target: T| {
            rects.iter().find_map(|(a, r)| {
                matches!(a, PropertyPanelAction::ToggleInteractionMenu(t) if *t == target)
                    .then_some(*r)
            })
        };
        let target = self.interaction_menu_target;
        let anchor = find(target).or_else(|| match target {
            // Only a trigger with no action yet has no row of its own; one
            // with several actions has no popover (only "Remove all").
            T::Trigger(t) if self.snapshot.interactions.actions(t).is_empty() => {
                find(T::AddTrigger)
            }
            _ => None,
        })?;
        // Dropdowns that sit in the right part of a row open right-aligned
        // to their own right edge (a narrow anchor does, see
        // `interaction_menu_rect`), so the 200 px list never hangs off the
        // panel's right side.
        Some(match target {
            T::MotionTrigger(_) | T::TransitionEasing => {
                let narrow = crate::widgets::property_panel_interactions_menu::NARROW_ANCHOR_W
                    .min(anchor.size.x);
                Rect {
                    origin: Point2D::new(anchor.origin.x + anchor.size.x - narrow, anchor.origin.y),
                    size: Point2D::new(narrow, anchor.size.y),
                }
            }
            _ => anchor,
        })
    }
}
