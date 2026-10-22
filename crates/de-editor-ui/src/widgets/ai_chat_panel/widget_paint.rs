//! `impl Widget for AIChatPlaceholder` — the panel's full composition
//! pass (collapsed pill, header, transcript, empty-state examples,
//! input block, bottom toolbar, overlays) plus its accessibility node.
//! Carved off `ai_chat_panel.rs` to keep every file under the 800-line
//! cap; layout + geometry stay on the spine.

use super::*;

/// Corner radius of the composer card.
const COMPOSER_RADIUS: f32 = 12.0;

impl<'a> Widget for AIChatPlaceholder<'a> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(self.state.panel_width, self.state.panel_height),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        // Minimized: the compact input bar replaces the whole panel.
        // Composer-only outranks it: that card IS the chat's resting
        // state on desktop now, so a stale `minimized` flag from an
        // earlier session must not swap it for the old bar.
        if self.state.is_minimized() && !self.composer_only {
            crate::widgets::ai_chat_panel_minimized::paint_minimized_bar(
                cx,
                &self.theme,
                self,
                rect,
            );
            return;
        }

        // Pinned, the chat IS the rail's Agent tab: the page background,
        // no card around it. Floating, it is a card over the canvas.
        if self.column_pinned {
            cx.backend.fill_rect(rect, self.palette.bg);
        } else {
            paint_panel_surface(cx, &self.theme, rect);
        }
        let can_use_model = !self.state.available_models.is_empty();

        // Composer-only: the rail is showing another tab, so the card
        // over the canvas is the input box and nothing else. Unfocused
        // it is the box alone; once the input has focus it grows the
        // slim header carrying the session name and the two ways into
        // the Agent tab. No transcript, no examples: the conversation
        // has one home and this is not it.
        if self.composer_only {
            if self.state.focused {
                crate::widgets::ai_chat_panel_header::paint_composer_header(
                    cx,
                    &self.theme,
                    rect,
                    &self.state.title,
                    self.header_hover,
                    self.header_pressed,
                );
            }
        } else {
            let header = self.header(rect);
            crate::widgets::ai_chat_panel_header::paint_session_header(
                cx,
                &self.palette,
                rect,
                &header,
                &crate::widgets::ai_chat_panel_header::SessionHeader {
                    title: &self.state.title,
                    new_label: &self.label_new,
                    menu_open: self.session_menu_open,
                    running: self.state.agents_running.0 > 0,
                    maximize_icon: self.maximize_icon(),
                    hover: self.header_hover,
                    pressed: self.header_pressed,
                    now_ms: self.now_ms,
                },
            );

            if self.state.messages.is_empty() {
                // The empty state is laid out from the top while the
                // composer block is bottom-anchored; clip to the region
                // between them so nothing overlaps the composer.
                let region = self.empty_state_region(rect);
                let t = |key| de_i18n::translate(self.locale, key);
                cx.backend.save();
                cx.backend.clip_rect(region);
                crate::widgets::ai_chat_panel_paint::paint_empty_state(
                    cx,
                    &self.palette,
                    rect,
                    &crate::widgets::ai_chat_panel_paint::EmptyState {
                        title: &self.label_start_with_ai,
                        examples: &self.examples,
                        tips: [
                            (
                                t("chat.tip.agent.title"),
                                t("chat.tip.agent.body").to_string(),
                            ),
                            (
                                t("chat.tip.attach.title"),
                                t("chat.tip.attach.body").to_string(),
                            ),
                            (
                                t("chat.tip.mcp.title"),
                                t("chat.tip.mcp.body").replace("{{endpoint}}", &self.mcp_endpoint),
                            ),
                        ],
                        // Prompts stay enabled without a connected model (#43) —
                        // clicking one fills the input; only streaming disables them.
                        disabled: self.is_streaming(),
                        hover: self.example_hover,
                        pressed: self.example_pressed,
                        content_bottom: region.origin.y + region.size.y,
                    },
                );
                cx.backend.restore();
            } else {
                let body = self.body_rect(rect);
                // Fingerprint + resolve the transcript ONCE for the whole paint
                // pass, then thread the build into both the scroll clamp and the
                // painter so neither re-hashes.
                let canonical =
                    crate::widgets::ai_chat_transcript_cache::cached_canonical_transcript_owned(
                        self.owner,
                        &self.state.messages,
                        body,
                        self.locale,
                    );
                let scroll_offset = crate::widgets::ai_chat_transcript::effective_offset_of(
                    &canonical,
                    body,
                    self.state.transcript_scroll.offset,
                    self.state.transcript_pinned,
                );
                crate::widgets::ai_chat_transcript::paint_transcript_with_selection(
                    cx,
                    &self.theme,
                    body,
                    &self.state.messages,
                    &canonical,
                    self.now_ms,
                    self.design_hover,
                    self.state.transcript_selection,
                    scroll_offset,
                );
            }
        }
        // Above even the chips: a turn that cannot start at all outranks
        // anything describing what the turn would do.
        if let (Some(row), Some(label)) = (self.mcp_notice_row(rect), self.mcp_notice.as_deref()) {
            crate::widgets::ai_chat_mcp_notice::paint_mcp_notice(cx, row, self.theme, label);
        }
        // The chips describe the next turn and what it is aimed at; they sit
        // above the composer card, not inside it.
        crate::widgets::ai_chat_chip_row::paint_chip_row(
            cx,
            &self.theme,
            self,
            self.input_rows_rect(rect),
        );

        // The composer card.
        let card = self.composer_card(rect);
        cx.backend
            .fill_round_rect(card, COMPOSER_RADIUS, self.palette.surface);
        cx.backend.stroke_round_rect(
            card,
            COMPOSER_RADIUS,
            if self.state.focused {
                crate::widgets::home_surface::fade(self.palette.accent, 0.6)
            } else {
                self.palette.border
            },
            1.0,
        );
        let input_rect = self.input_text_rect(rect);
        let input_area_h = input_rect.size.y;
        crate::widgets::ai_chat_input_text::paint_input_text_area(
            cx,
            &self.theme,
            self.state,
            input_rect,
            input_area_h,
            self.now_ms,
            &self.label_input_placeholder,
        );

        // Staged-attachment strip — between the text and the bottom row,
        // present only when attachments are staged.
        let attach_h = self.attachment_row_h();
        if attach_h > 0.0 {
            let attach_rect = Rect {
                origin: Point2D::new(input_rect.origin.x, input_rect.origin.y + input_area_h),
                size: Point2D::new(input_rect.size.x, attach_h),
            };
            paint_attachment_row(cx, &self.theme, attach_rect, self.state);
        }

        let footer = self.footer_layout(rect);
        let streaming = self.is_streaming();
        let send_active = can_use_model
            && (!self.state.input.text().trim().is_empty()
                || !self.state.pending_attachments.is_empty());
        paint_bottom_toolbar(cx, self, &footer, streaming, send_active);

        // Dropdowns paint above everything else in the panel.
        if let Some(picker) = self.model_picker_bounds(rect) {
            crate::widgets::ai_chat_model_picker::paint_model_picker(
                cx,
                &self.theme,
                picker,
                &self.state.available_models,
                &self.offline_agents,
                self.state.selected_model,
                self.model_picker,
                self.model_picker_input,
                self.now_ms,
                self.locale,
            );
        }
        if let Some(menu) = self.session_menu_bounds(rect) {
            let titles: Vec<String> = self.tabs_snapshot.iter().map(|t| t.title.clone()).collect();
            crate::widgets::ai_chat_session_menu::paint_session_menu(
                cx,
                &self.palette,
                menu,
                &titles,
                self.active_tab_index,
                self.tab_hover,
            );
        }
        if let Some(menu) = self.plus_menu_bounds(rect) {
            crate::widgets::ai_chat_plus_menu::paint_plus_menu(
                cx,
                &self.palette,
                menu,
                &crate::widgets::ai_chat_plus_menu::PlusMenu {
                    thinking: self.state.thinking_mode,
                    agent_team_size: self.state.agent_team_size,
                    streaming,
                    hover: self.plus_menu_hover,
                    locale: self.locale,
                },
            );
        }
        if self.parallel_agents_picker_open {
            crate::widgets::ai_chat_panel_footer::paint_parallel_agents_picker(
                cx,
                &self.theme,
                &footer,
                self.state.agent_team_size,
                self.parallel_agents_picker_hover,
            );
        }

        // Pinned-style detail card — last of all, so it hangs over the input
        // block, and anchored ABOVE its chip so the ✕ on that row stays both
        // visible and clickable. Present only once the dwell has elapsed; the
        // resolve that builds it is gated on the same clock.
        if let Some(card) = self.style_card.as_ref() {
            if let Some(chip) = self.chip_row(self.input_rows_rect(rect)).style {
                crate::widgets::ai_chat_style_card::paint_style_card(
                    cx,
                    &self.theme,
                    card,
                    chip,
                    rect,
                    self.locale,
                );
            }
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Group);
        node.set_label(de_i18n::translate(self.locale, "a11y.aiChat"));
        node
    }
}
