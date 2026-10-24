//! `PropertyPanel` painting — the `Widget` impl (tab strip, section
//! stack, action feedback wash, every anchored popover) plus the
//! out-of-rail overlay pass hosts call late in composition.
//!
//! Split out of `property_panel.rs` to keep both files under the
//! design-editor 800-line cap.

use super::{action_wash_rect, PropertyPanel, PropertyPanelAction};
use crate::widgets::button::paint_button_feedback_wash;
use crate::widgets::property_panel_sections as sections;
use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Point2D, Rect};

impl Widget for PropertyPanel {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, cx: &LayoutCx) -> LayoutBox {
        // Vertical extent is "as much as you give me" — the host
        // clips at the rail rect. Reporting 800 here is just a
        // placeholder for the abstract widget tree.
        LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(cx.available_width, 800.0),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        // The hairline is the floating card's (#86): hosts paint the panel
        // through `floating_panels::paint_card`.
        cx.backend.fill_rect(rect, self.theme.card);

        let x = rect.origin.x;
        let w = rect.size.x;
        // The header is pinned to the panel top — painted fixed, above
        // (and never scrolled with) the section content.
        {
            use crate::widgets::property_panel_header::{
                node_header, paint_panel_header, PanelHeader,
            };
            let code = matches!(self.tab, de_editor_core::PropertyTab::Code);
            let page_kind = de_i18n::translate(self.locale, "property.page");
            let header = if code {
                PanelHeader::Code {
                    title: self.labels.tab_code,
                }
            } else if self.page_only {
                PanelHeader::Node {
                    icon: crate::widgets::icons::Icon::FileText,
                    accent: self.theme.primary,
                    title: if self.page_name.is_empty() {
                        page_kind
                    } else {
                        self.page_name.as_str()
                    },
                    kind: page_kind,
                    create_component: false,
                }
            } else {
                node_header(&self.theme, &self.snapshot, self.header_create_component())
            };
            let close_hovered = code && self.close_hovered;
            paint_panel_header(cx, &self.theme, rect, &header, close_hovered);
        }
        let tab_bottom = rect.origin.y + crate::widgets::property_panel_inputs::PANEL_HEADER_HEIGHT;
        let edit_ctx = sections::EditContext {
            focus: self.focus,
            draft: self.draft.as_str(),
            input: &self.input,
            caret: self.caret_pos,
            select_all: self.select_all,
            now_ms: self.now_ms,
        };
        let caps = self.capabilities();
        if matches!(self.tab, de_editor_core::PropertyTab::Code) {
            crate::widgets::property_panel_code::paint_code_panel_in_panel_with_locale_pressed_and_touch(
                cx,
                &self.theme,
                &self.codegen,
                self.locale,
                rect,
                self.now_ms,
                self.codegen_pressed,
            );
            return;
        }
        if self.page_only {
            cx.backend.save();
            cx.backend.clip_rect(Rect {
                origin: Point2D::new(x, tab_bottom),
                size: Point2D::new(w, (rect.origin.y + rect.size.y - tab_bottom).max(0.0)),
            });
            crate::widgets::property_panel_page::paint_page_inspector(
                cx,
                &self.theme,
                &edit_ctx,
                self.locale,
                self.page_background.as_deref(),
                rect,
            );
            cx.backend.restore();
            return;
        }
        // Section content scrolls below the pinned tab strip; clip it
        // so a scrolled-up section can't paint over the tabs or bleed
        // onto the neighbouring rail. Overlays (fill / export pickers)
        // anchor to `scrolled` — the same shifted rect the layout
        // walker uses (it adds `PANEL_HEADER_HEIGHT`), so paint + hit-test of
        // the sections agree.
        cx.backend.save();
        cx.backend.clip_rect(Rect {
            origin: Point2D::new(x, tab_bottom),
            size: Point2D::new(w, (rect.origin.y + rect.size.y - tab_bottom).max(0.0)),
        });
        let scroll = self.effective_scroll(rect);
        let scrolled = Rect {
            origin: Point2D::new(rect.origin.x, rect.origin.y - scroll),
            size: rect.size,
        };
        // First section sits just below the pinned tab strip:
        // `tab_bottom - scroll` == `scrolled.origin.y + PANEL_HEADER_HEIGHT`,
        // matching the layout walker's `+= PANEL_HEADER_HEIGHT` step.
        let mut y = tab_bottom - scroll;
        // Each section paints from its shared top (#83) — the origin the
        // action and input walkers use — and debug builds check that the
        // section above ended exactly there.
        let tops = crate::widgets::property_panel_section_tops::section_tops(
            scrolled.origin.y,
            self.visible_sections(),
            &self.snapshot.effects,
            &self.snapshot.fills,
            &self.snapshot.interactions,
        );
        if self.visible_sections().create_component {
            y = crate::widgets::property_panel_instance::paint_component_block(
                cx,
                &self.theme,
                &self.labels,
                self.visible_sections().component_button,
                &self.instance_component_options,
                self.instance_component_target.as_deref(),
                tab_bottom,
                rect.origin.y + rect.size.y,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.position, "component block");
        y = sections::paint_position_section(
            cx,
            &self.theme,
            &self.snapshot,
            &edit_ctx,
            &self.labels,
            self.snapshot.has_corner_radius,
            self.corner_expand_open,
            caps.size_options,
            x,
            y,
            w,
        );
        // Multi-select panel presses are inert (`hit.rs`), so the buttons
        // only show for one node; Shift+H / Shift+V flip a multi-selection.
        if !self.is_multi {
            crate::widgets::property_panel_flip::paint_flip_buttons(
                cx,
                &self.theme,
                &self.snapshot,
                x,
                tops.position,
                w,
            );
        }
        y = section_start(y, tops.flex, "position");
        let flex_section_y = y;
        if caps.flex_layout {
            y = crate::widgets::property_panel_flex::paint_flex_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                &self.labels,
                self.locale,
                // The sizing boxes close the section when they follow.
                !caps.size_options,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.size, "auto layout");
        if caps.size_options {
            y = sections::paint_size_section(
                cx,
                &self.theme,
                &self.labels,
                self.size_flags,
                self.snapshot.can_clip_content,
                !caps.flex_layout,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.icon, "size");
        if self.snapshot.icon.is_some() {
            y = crate::widgets::property_panel_icon::paint_icon_section(
                cx,
                &self.theme,
                &self.snapshot,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.text, "icon");
        if caps.text && self.snapshot.text.is_some() {
            y = crate::widgets::property_panel_text::paint_text_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.widget, "text");
        if self.snapshot.widget.is_some() {
            y = crate::widgets::property_panel_widget::paint_widget_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.annotation, "widget");
        if self.snapshot.annotation.is_some() {
            y = crate::widgets::property_panel_annotation::paint_annotation_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.image, "annotation");
        if caps.image && self.snapshot.is_image_node {
            y = crate::widgets::property_panel_image_node::paint_image_node_section(
                cx,
                &self.theme,
                &self.snapshot,
                self.image_panel_view
                    .as_ref()
                    .and_then(|v| v.warning.as_ref()),
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.video, "image");
        if caps.image && self.snapshot.video.is_some() {
            y = crate::widgets::property_panel_video::paint_video_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.layer, "video");
        if caps.opacity {
            y = sections::paint_layer_section(
                cx,
                &self.theme,
                &self.snapshot,
                &self.labels,
                &edit_ctx,
                !self.is_multi,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.fill, "layer");
        if caps.fill {
            y = sections::paint_fill_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                &self.labels,
                self.visible_sections(),
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.stroke, "fill");
        let stroke_section_y = y;
        if caps.stroke {
            y = crate::widgets::property_panel_stroke::paint_stroke_section(
                cx,
                &self.theme,
                &self.snapshot,
                &edit_ctx,
                &self.labels,
                self.stroke_variable_ref.as_deref(),
                self.color_variable_count > 0 || self.stroke_variable_ref.is_some(),
                x,
                y,
                w,
                self.stroke_edit_mode,
            );
        }
        y = section_start(y, tops.effects, "stroke");
        if caps.effects {
            y = sections::paint_effects_section(
                cx,
                &self.theme,
                &self.labels,
                &self.snapshot.effects,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.interactions, "effects");
        if caps.interactions {
            y = sections::paint_interactions_section(
                cx,
                &self.theme,
                &self.snapshot.interactions,
                &edit_ctx,
                self.locale,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.export, "interactions");
        if caps.export {
            y = sections::paint_export_section(
                cx,
                &self.theme,
                &self.labels,
                self.export_format,
                self.export_scale,
                self.export_section_open,
                x,
                y,
                w,
            );
        }
        y = section_start(y, tops.code, "export");
        crate::widgets::property_panel_code_chips::paint_code_chips_section(
            cx,
            &self.theme,
            self.labels.tab_code,
            self.codegen.framework,
            x,
            y,
            w,
        );
        // Base-action feedback belongs to the scrolling panel body. Paint it
        // before every floating menu so a stale hover/press wash can never be
        // composited over popup chrome. The menu-specific hover states below
        // then remain the sole feedback on the top-most surface.
        if self.action_hover.is_some() || self.action_pressed.is_some() {
            let rects = sections::action_button_rects_with_fill_picker(
                self.scrolled_rect(rect),
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
            if let Some(i) = self.action_hover {
                if let Some((action, r)) = rects.get(i) {
                    let wash = action_wash_rect(action, *r, &self.labels, cx.backend);
                    paint_button_feedback_wash(
                        cx.backend,
                        &self.theme,
                        wash,
                        6.0,
                        true,
                        self.action_pressed == Some(i),
                    );
                    if matches!(action, PropertyPanelAction::ToggleCornerExpand) {
                        crate::widgets::property_panel_corner::paint_tooltip(
                            cx,
                            &self.theme,
                            *r,
                            self.labels.corner_per_corner,
                        );
                    }
                }
            }
            if let Some(i) = self.action_pressed {
                if self.action_hover != Some(i) {
                    if let Some((action, r)) = rects.get(i) {
                        let wash = action_wash_rect(action, *r, &self.labels, cx.backend);
                        paint_button_feedback_wash(cx.backend, &self.theme, wash, 6.0, false, true);
                    }
                }
            }
        }
        // Effects "+" add-menu overlay.
        if self.effect_add_picker_open {
            if let Some(add_rect) = self.effect_add_button_rect(scrolled) {
                crate::widgets::property_panel_effects::paint_effect_add_menu(
                    cx,
                    &self.theme,
                    &self.labels,
                    add_rect,
                    self.effect_add_menu_hover,
                );
            }
        }
        // The shared Interactions / Motion popover.
        if caps.interactions && self.interaction_menu_open {
            if let Some(anchor) = self.interaction_menu_anchor_rect(scrolled) {
                let rows = self.interaction_menu_rows();
                crate::widgets::property_panel_interactions::paint_interaction_menu(
                    cx,
                    &self.theme,
                    anchor,
                    crate::widgets::property_panel_interactions::interaction_menu_viewport(rect),
                    &rows,
                    self.interaction_menu_hover,
                );
            }
        }
        // A fill row's `⋯` menu — under the fill-type / blend pickers
        // it opens, over everything else.
        if caps.fill {
            if let Some(index) = self.fill_menu_open {
                let button = self
                    .popup_action_rects(rect)
                    .into_iter()
                    .find_map(|(a, r)| {
                        matches!(a, PropertyPanelAction::ToggleFillMenu(i) if i == index)
                            .then_some(r)
                    });
                if let (Some(button), Some(fill)) = (button, self.snapshot.fills.get(index)) {
                    crate::widgets::property_panel_fill_menu::paint_fill_menu(
                        cx,
                        &self.theme,
                        self.locale,
                        fill,
                        index,
                        self.snapshot.fills.len(),
                        button,
                        rect.size.x,
                    );
                }
            }
        }
        // Fill-type picker overlay sits on top of everything below
        // the Fill section so it can extend past the section divider.
        if caps.fill && self.fill_type_picker.open {
            let fi = self.fill_type_picker_index;
            if let Some(action_rect) = sections::fill_type_toggle_action_rect(
                scrolled,
                self.visible_sections(),
                &self.snapshot.effects,
                &self.snapshot.fills,
                fi,
            ) {
                let active = self
                    .snapshot
                    .fills
                    .get(fi)
                    .map(|f| f.fill_type)
                    .unwrap_or(self.fill_type);
                sections::paint_fill_type_picker(
                    cx,
                    &self.theme,
                    action_rect,
                    crate::widgets::property_panel_fill::fill_type_picker_viewport(rect),
                    &self.fill_type_picker,
                    active,
                    self.locale,
                );
            }
        }
        if caps.text && self.font_picker.open {
            if let Some(text) = self.snapshot.text.as_ref() {
                let entries = self.font_picker_entries();
                crate::widgets::property_panel_typography::paint_font_picker(
                    cx,
                    &self.theme,
                    scrolled,
                    self.visible_sections(),
                    self.locale,
                    &entries,
                    self.font_import_supported,
                    &self.font_picker_search,
                    &self.font_picker,
                    self.font_picker_import_hover,
                    &text.font_family,
                    self.now_ms,
                );
            }
        }
        if caps.text && self.font_weight_picker_open {
            if let Some(text) = self.snapshot.text.as_ref() {
                crate::widgets::property_panel_text::paint_font_weight_picker(
                    cx,
                    &self.theme,
                    scrolled,
                    self.visible_sections(),
                    self.locale,
                    text.font_weight,
                    self.font_weight_picker_hover,
                    self.font_weight_picker_pressed,
                );
            }
        }
        // Alignment popover — overlays the sections below the Auto
        // layout header. Anchored off the section body top, the same y
        // the action walker passes to `push_flex_action_rects`.
        if caps.flex_layout && self.layout_align_popover_open {
            crate::widgets::property_panel_flex::paint_layout_align_popover(
                cx,
                &self.theme,
                self.locale,
                &self.snapshot,
                self.layout_align_popover_hover,
                x,
                flex_section_y + crate::widgets::property_panel_inputs::SECTION_HEADER_HEIGHT,
                w,
            );
        }
        if caps.stroke && self.stroke_mode_popover_open {
            crate::widgets::property_panel_stroke::paint_stroke_mode_popover(
                cx,
                &self.theme,
                self.locale,
                self.stroke_edit_mode,
                self.stroke_mode_popover_hover,
                x,
                stroke_section_y,
                w,
            );
        }
        // Export-section inline select popups — painted last so the
        // scale / format dropdown overlays sit above every section.
        if caps.export && (self.export_scale_picker_open || self.export_format_picker_open) {
            sections::paint_export_picker(
                cx,
                &self.theme,
                scrolled,
                self.visible_sections(),
                &self.snapshot.effects,
                &self.snapshot.fills,
                &self.snapshot.interactions,
                self.export_scale_picker_open,
                self.export_format_picker_open,
                self.export_scale,
                self.export_format,
                self.export_picker_hover,
            );
        }
        // The colour-variable picker paints as a floating popover over the
        // visible rail, so a long variable list scrolls inside its own
        // capped box instead of stretching the inspector.
        if let Some(layout) = self.color_variable_picker_layout_logical(rect) {
            crate::widgets::property_panel_color_variables::paint_color_variable_picker(
                cx,
                &self.theme,
                &layout,
                &self.color_variables,
                self.bound_color_variable_ref(),
                self.color_variable_picker_hover,
                self.locale,
            );
        }
        self.paint_compositing_picker(cx, rect);
        cx.backend.restore();
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Group);
        node.set_label(if self.page_only {
            self.page_name.clone()
        } else {
            self.snapshot.kind.clone()
        });
        node
    }
}

impl PropertyPanel {
    /// Paint inspector overlays that are allowed to extend out of the
    /// right rail. Hosts call this late in their composition pass so
    /// the image-fill / search / generate popovers sit above floating
    /// canvas controls.
    pub fn paint_overlays(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        if self.page_only {
            return;
        }
        let caps = self.capabilities();
        if !(caps.fill || caps.image) {
            return;
        }
        // The Code tab paints no Design sections — none of the
        // Design-anchored popovers may float over it.
        if matches!(self.tab, de_editor_core::PropertyTab::Code) {
            return;
        }
        let scroll = self.effective_scroll(rect);
        let scrolled = Rect {
            origin: Point2D::new(rect.origin.x, rect.origin.y - scroll),
            size: rect.size,
        };
        if self.image_fill_popover_open {
            let edit_ctx = sections::EditContext {
                focus: self.focus,
                draft: self.draft.as_str(),
                input: &self.input,
                caret: self.caret_pos,
                select_all: self.select_all,
                now_ms: self.now_ms,
            };
            sections::paint_image_fill_popover(
                cx,
                &self.theme,
                scrolled,
                self.visible_sections(),
                &self.snapshot,
                &edit_ctx,
                self.locale,
            );
        }
        if caps.image && self.image_panel.search_open {
            crate::widgets::property_panel_image_popovers::paint_search_popover(
                cx,
                &self.theme,
                scrolled,
                self.visible_sections(),
                &self.image_panel,
                self.locale,
                self.now_ms,
            );
        }
        if caps.image && self.image_panel.generate_open {
            crate::widgets::property_panel_image_popovers::paint_generate_popover(
                cx,
                &self.theme,
                scrolled,
                self.visible_sections(),
                &self.image_panel,
                self.image_gen_profile.as_ref(),
                self.locale,
                self.now_ms,
            );
        }
    }
}

/// Paint resumes each section at its shared top. In debug builds, check
/// that the section above (`above`) ended exactly there, so a paint
/// routine whose height drifts from `section_tops` fails every panel
/// paint test instead of silently misaligning hit targets.
fn section_start(painted_end: f32, top: f32, above: &str) -> f32 {
    debug_assert!(
        (painted_end - top).abs() < 0.5,
        "{above} section painted to {painted_end}, but the next section starts at {top}"
    );
    top
}
