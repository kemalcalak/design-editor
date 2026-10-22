//! Hit-testing + resize-edge geometry for the AI chat panel — split
//! out of `ai_chat_panel.rs` to keep that file under the 800-line cap.
//! Pure geometry; the painting half stays in `ai_chat_panel.rs`.

use super::ai_chat_panel::{AIChatPlaceholder, RESIZE_CORNER, RESIZE_GUTTER};
use crate::widgets::ai_chat_hit::{AIChatHit, ChatCursorProbe, ChatResizeEdge};
use crate::widgets::ai_chat_panel_controls::attachment_row_hit;
use crate::widgets::ai_chat_panel_paint::example_card_rects;
use crate::widgets::ai_chat_transcript_cache::CanonicalTranscript;
use crate::{Point2D, Rect};

impl<'a> AIChatPlaceholder<'a> {
    /// Resolve a hit owned by the open model-picker overlay.
    ///
    /// The picker grows upward and can extend beyond (or overlap the header
    /// of) the chat panel. Keep this probe separate from the panel-body
    /// containment check so every painted part of the overlay stays
    /// interactive and wins over controls painted underneath it.
    fn open_model_picker_hit(&self, rect: Rect, point: Point2D) -> Option<AIChatHit> {
        if self.state.is_minimized() || !self.model_picker.open {
            return None;
        }
        let picker = self.model_picker_bounds(rect)?;
        if let Some(provider) = crate::widgets::ai_chat_model_picker::offline_agent_at(
            self.model_picker,
            picker,
            point,
            &self.state.available_models,
            &self.offline_agents,
            self.model_picker_input.text(),
        ) {
            return Some(AIChatHit::ConnectAgent(provider));
        }
        if crate::widgets::ai_chat_model_picker::footer_rect(picker).contains(point) {
            return Some(AIChatHit::OpenProviderSettings);
        }
        if crate::widgets::ai_chat_model_picker::search_clear_hit(
            picker,
            point,
            self.model_picker_input.text(),
        ) {
            return Some(AIChatHit::ClearModelSearch);
        }
        match crate::widgets::ai_chat_model_picker::model_picker_hit(
            self.model_picker,
            picker,
            point,
            &self.state.available_models,
            &self.offline_agents,
            self.model_picker_input.text(),
        ) {
            jian_widgets::components::select::SelectHit::Row(idx) => {
                Some(AIChatHit::SelectModel(idx))
            }
            jian_widgets::components::select::SelectHit::Inside => {
                Some(AIChatHit::FocusModelSearch)
            }
            jian_widgets::components::select::SelectHit::Outside => None,
        }
    }

    pub fn hit_test(&self, rect: Rect, point: Point2D) -> Option<AIChatHit> {
        self.hit_test_with_canonical(rect, point, None)
    }

    /// Hit-test with an optionally pre-resolved canonical transcript. When
    /// `canonical` is `Some`, the transcript branch reuses it instead of
    /// resolving (and re-fingerprinting) its own — this is how
    /// [`Self::cursor_probe`] makes one cursor event fingerprint the transcript
    /// at most once. `None` resolves it on demand (the plain `hit_test` path).
    fn hit_test_with_canonical(
        &self,
        rect: Rect,
        point: Point2D,
        canonical: Option<&CanonicalTranscript>,
    ) -> Option<AIChatHit> {
        if let Some(hit) = self.open_model_picker_hit(rect, point) {
            return Some(hit);
        }
        if let Some(hit) = self.open_menu_hit(rect, point) {
            return Some(hit);
        }
        if let Some(edge) = self.resize_edge_at(rect, point) {
            return Some(AIChatHit::Resize(edge));
        }
        if !(rect).contains(point) {
            return None;
        }
        // When minimized: anywhere on the compact bar expands it. The bar
        // carries no interactive sub-controls and cannot be dragged — a
        // single unambiguous target beats splitting a 64 px-tall strip
        // between drag intent, model switching and expand intent.
        if self.state.is_minimized() && !self.composer_only {
            return Some(AIChatHit::ToggleCollapse);
        }
        let can_use_model = !self.state.available_models.is_empty();
        // Composer-only: nothing above the composer exists except the
        // focused card's slim header, whose two glyphs both mean "take
        // me to the Agent tab".
        if self.composer_only {
            if self.state.focused {
                use crate::widgets::ai_chat_panel::COMPOSER_HEADER_HEIGHT;
                let header = Rect {
                    origin: rect.origin,
                    size: Point2D::new(rect.size.x, COMPOSER_HEADER_HEIGHT),
                };
                if header.contains(point) {
                    return Some(AIChatHit::ToggleMaximize);
                }
            }
        } else {
            // Hit rects come from the same `header_layout` paint reads.
            let header = self.header(rect);
            if header.new_chat.contains(point) {
                return Some(AIChatHit::NewChat);
            }
            if header.maximize.is_some_and(|m| m.contains(point)) {
                return Some(AIChatHit::ToggleMaximize);
            }
            if header.select.contains(point) {
                return Some(AIChatHit::ToggleSessionMenu);
            }
        }
        // Must match `paint` exactly: paint draws the separator at
        // `bottom - input_h` and the input block one pixel below it
        // (`sep_y + 1`). An earlier `- PAD` here put the hit targets
        // ~17 px above where they are painted.
        let input_rect = self.input_rect(rect);
        let input_area_h = self.input_area_height_for_rect(rect);
        // The open picker was probed before panel containment because it can
        // extend beyond this rect. Reaching here means the point is inside the
        // chat but outside the picker, so dismiss the modal overlay after any
        // header control that was not visually covered by it has handled the
        // press normally.
        if self.model_picker.open {
            return Some(AIChatHit::ToggleModelPicker);
        }
        if (input_rect).contains(point) {
            // Pre-flight MCP notice — the actual topmost band when it shows,
            // and a button: clicking it opens Settings on the MCP tab. Every
            // row below is measured from `rows_rect`, the input block with
            // the notice already taken off the top, so their geometry is
            // identical whether or not the notice is up.
            if let Some(notice) = self.mcp_notice_row(rect) {
                if crate::widgets::ai_chat_mcp_notice::notice_rect(notice).contains(point) {
                    return Some(AIChatHit::OpenMcpSettings);
                }
            }
            let notice_h = self.mcp_notice_row_h();
            let input_rect = Rect {
                origin: Point2D::new(input_rect.origin.x, input_rect.origin.y + notice_h),
                size: Point2D::new(input_rect.size.x, (input_rect.size.y - notice_h).max(0.0)),
            };
            // Topmost band of the input block, so it is tested first. Both
            // chips share the row, so both ✕ targets are probed here — the
            // rects come from the same `chip_row` paint draws from.
            let chip_row_h = self.chip_row_h();
            let chip_bottom = input_rect.origin.y + chip_row_h;
            let text_area = self.input_text_rect(rect);
            let text_top = text_area.origin.y;
            let attach_top = text_top + input_area_h;
            let attach_h = self.attachment_row_h();
            let toolbar_top = attach_top + attach_h;
            if chip_row_h > 0.0 && point.y < chip_bottom {
                if self
                    .style_receipt_clear_rect(input_rect)
                    .is_some_and(|clear| clear.contains(point))
                {
                    return Some(AIChatHit::ClearPinnedStyle);
                }
                if self
                    .selection_chip_clear_rect(input_rect)
                    .is_some_and(|clear| clear.contains(point))
                {
                    return Some(AIChatHit::ClearSelection);
                }
                return Some(AIChatHit::FocusInput);
            }
            if point.y < attach_top {
                if self.is_streaming() {
                    return Some(AIChatHit::Inside);
                }
                if self.state.input.text().is_empty() {
                    return Some(AIChatHit::FocusInput);
                }
                let offset = crate::widgets::ai_chat_input_text::input_text_offset_at(
                    self.state, text_area, point,
                )
                .unwrap_or(self.state.input.text().len());
                return Some(AIChatHit::SelectInputText(offset));
            }
            // Staged-attachment strip — present only when attachments
            // are staged; a chip click removes that attachment.
            if attach_h > 0.0 && point.y >= attach_top && point.y < toolbar_top {
                let row = Rect {
                    origin: Point2D::new(text_area.origin.x, attach_top),
                    size: Point2D::new(text_area.size.x, attach_h),
                };
                if let Some(hit) =
                    attachment_row_hit(row, point, self.state.pending_attachments.len())
                {
                    return Some(hit);
                }
                return Some(AIChatHit::FocusInput);
            }
            // The card's bottom row: "+" | model chip | send / stop.
            if point.y >= toolbar_top {
                let footer = self.footer_layout(rect);
                let streaming = self.is_streaming();
                if footer.plus.contains(point) {
                    return Some(AIChatHit::TogglePlusMenu);
                }
                if footer.model.size.x > 0.0 && footer.model.contains(point) {
                    return Some(if !self.usable_agent {
                        AIChatHit::OpenProviderSettings
                    } else if can_use_model {
                        AIChatHit::ToggleModelPicker
                    } else {
                        AIChatHit::FocusInput
                    });
                }
                // Stop shares the send slot and is live only while streaming.
                if streaming && footer.stop.contains(point) {
                    return Some(AIChatHit::Stop);
                }
                if footer.send.contains(point) {
                    return Some(
                        if can_use_model
                            && !streaming
                            && (!self.state.input.text().trim().is_empty()
                                || !self.state.pending_attachments.is_empty())
                        {
                            AIChatHit::Send
                        } else {
                            AIChatHit::FocusInput
                        },
                    );
                }
            }
            return Some(if self.is_streaming() {
                AIChatHit::Inside
            } else {
                AIChatHit::FocusInput
            });
        }
        // Transcript hit-test — a click on a message's thinking /
        // tool-call collapsible header toggles it. Checked before the
        // drag-handle fallback so the headers are interactive.
        if !self.state.messages.is_empty() {
            let body = self.body_rect(rect);
            // Fingerprint + resolve the transcript ONCE for this event, then
            // thread the build into the scroll clamp and every transcript
            // probe below so none of them re-hashes. `cursor_probe` passes a
            // build it already resolved so the whole cursor event hashes once.
            let resolved;
            let canonical: &CanonicalTranscript = match canonical {
                Some(c) => c,
                None => {
                    resolved =
                        crate::widgets::ai_chat_transcript_cache::cached_canonical_transcript_owned(
                            self.owner,
                            &self.state.messages,
                            body,
                            self.locale,
                        );
                    &resolved
                }
            };
            let scroll_offset = crate::widgets::ai_chat_transcript::effective_offset_of(
                canonical,
                body,
                self.state.transcript_scroll.offset,
                self.state.transcript_pinned,
            );
            if let Some(hit) = crate::widgets::ai_chat_transcript::transcript_text_offset_at(
                &self.state.messages,
                canonical,
                body,
                point,
                scroll_offset,
            ) {
                return Some(AIChatHit::SelectTranscriptText(
                    hit.message_index,
                    hit.offset,
                ));
            }
            if let Some(hit) = crate::widgets::ai_chat_transcript::transcript_hit(
                canonical,
                body,
                point.x,
                point.y,
                scroll_offset,
            ) {
                return Some(hit.into());
            }
        }
        if self.state.messages.is_empty() && !self.is_streaming() {
            // Examples grid hit-test (only rendered when no messages).
            // Clickable regardless of model connection — clicking an example
            // fills the input (sending separately requires a model) (#43).
            // Pills the shrunk sheet dropped from paint (they would overlap
            // the composer) are not click targets either.
            let region = self.empty_state_region(rect);
            let content_bottom = region.origin.y + region.size.y;
            for (index, (card, ex)) in example_card_rects(rect)
                .iter()
                .zip(self.examples.iter())
                .enumerate()
            {
                if crate::widgets::ai_chat_panel_paint::example_card_fits(card, content_bottom)
                    && (*card).contains(point)
                {
                    return Some(AIChatHit::Example {
                        index,
                        prompt: ex.prompt.clone(),
                    });
                }
            }
        }
        if self.state.maximized {
            return Some(AIChatHit::FocusInput);
        }
        Some(AIChatHit::DragHandle)
    }

    pub fn resize_edge_at(&self, rect: Rect, point: Point2D) -> Option<ChatResizeEdge> {
        if self.state.is_minimized() || self.state.maximized {
            return None;
        }
        let left = rect.origin.x;
        let right = rect.origin.x + rect.size.x;
        let top = rect.origin.y;
        let bottom = rect.origin.y + rect.size.y;
        let outer = Rect::xywh(
            left - RESIZE_GUTTER,
            top - RESIZE_GUTTER,
            rect.size.x + RESIZE_GUTTER * 2.0,
            rect.size.y + RESIZE_GUTTER * 2.0,
        );
        if !(outer).contains(point) {
            return None;
        }

        let near_top = (point.y - top).abs() <= RESIZE_GUTTER;
        let near_bottom = (point.y - bottom).abs() <= RESIZE_GUTTER;
        let near_left = (point.x - left).abs() <= RESIZE_GUTTER;
        let near_right = (point.x - right).abs() <= RESIZE_GUTTER;
        let in_left_corner = point.x <= left + RESIZE_CORNER;
        let in_right_corner = point.x >= right - RESIZE_CORNER;
        let in_top_corner = point.y <= top + RESIZE_CORNER;
        let in_bottom_corner = point.y >= bottom - RESIZE_CORNER;

        match (
            near_top && in_left_corner,
            near_top && in_right_corner,
            near_bottom && in_left_corner,
            near_bottom && in_right_corner,
        ) {
            (true, _, _, _) => return Some(ChatResizeEdge::Nw),
            (_, true, _, _) => return Some(ChatResizeEdge::Ne),
            (_, _, true, _) => return Some(ChatResizeEdge::Sw),
            (_, _, _, true) => return Some(ChatResizeEdge::Se),
            _ => {}
        }
        if near_top {
            Some(ChatResizeEdge::N)
        } else if near_bottom {
            Some(ChatResizeEdge::S)
        } else if near_left && !in_top_corner && !in_bottom_corner {
            Some(ChatResizeEdge::W)
        } else if near_right && !in_top_corner && !in_bottom_corner {
            Some(ChatResizeEdge::E)
        } else {
            None
        }
    }

    pub fn design_block_hover_at(&self, rect: Rect, point: Point2D) -> Option<(usize, usize)> {
        self.design_block_hover_with_canonical(rect, point, None)
    }

    /// Design-block hover with an optionally pre-resolved canonical transcript.
    /// [`Self::cursor_probe`] passes the build it already resolved so the hover
    /// probe reuses it instead of fingerprinting the transcript again.
    fn design_block_hover_with_canonical(
        &self,
        rect: Rect,
        point: Point2D,
        canonical: Option<&CanonicalTranscript>,
    ) -> Option<(usize, usize)> {
        if self.state.messages.is_empty() {
            return None;
        }
        let body = self.body_rect(rect);
        // One fingerprint + resolve for this hover event; the scroll clamp and
        // the design-block probe both read the resolved build.
        let resolved;
        let canonical: &CanonicalTranscript = match canonical {
            Some(c) => c,
            None => {
                resolved =
                    crate::widgets::ai_chat_transcript_cache::cached_canonical_transcript_owned(
                        self.owner,
                        &self.state.messages,
                        body,
                        self.locale,
                    );
                &resolved
            }
        };
        let scroll_offset = crate::widgets::ai_chat_transcript::effective_offset_of(
            canonical,
            body,
            self.state.transcript_scroll.offset,
            self.state.transcript_pinned,
        );
        crate::widgets::ai_chat_transcript_hit::design_block_at(
            canonical,
            body,
            point.x,
            point.y,
            scroll_offset,
        )
    }

    /// Combined per-cursor-event probe: resolve the canonical transcript layout
    /// ONCE and return both the hit under the cursor and the design-block hover.
    ///
    /// Hosts drive one physical cursor move through the header-hover hit-test
    /// and the design-block hover (and native re-runs the hit-test from its
    /// cursor-hint pass). Calling those separately fingerprinted the whole
    /// transcript two or three times. Resolving the build here and threading it
    /// into both sub-probes collapses that to a single fingerprint per event;
    /// the host feeds `hit` to the header-hover / cursor-hint updates and
    /// `design_block_hover` to the design-hover update. Value-hashing stays the
    /// correctness anchor — there are no pointer / length shortcuts.
    pub fn cursor_probe(&self, rect: Rect, point: Point2D) -> ChatCursorProbe {
        if self.state.messages.is_empty() {
            // No transcript to resolve — the hit-test still handles the header,
            // input and overlays, and there is no design block to hover.
            return ChatCursorProbe {
                hit: self.hit_test_with_canonical(rect, point, None),
                design_block_hover: None,
            };
        }
        let body = self.body_rect(rect);
        let canonical = crate::widgets::ai_chat_transcript_cache::cached_canonical_transcript_owned(
            self.owner,
            &self.state.messages,
            body,
            self.locale,
        );
        ChatCursorProbe {
            hit: self.hit_test_with_canonical(rect, point, Some(&canonical)),
            design_block_hover: self.design_block_hover_with_canonical(
                rect,
                point,
                Some(&canonical),
            ),
        }
    }

    /// Cursor-shape hit-test that reads the LAST BUILT (= last painted)
    /// transcript layout WITHOUT fingerprinting or rebuilding anything.
    ///
    /// The native cursor-hint pass runs at raw-event time, before the deferred
    /// `apply_cursor_move` resolves (and repaints) the transcript. Rather than
    /// fingerprint the transcript a second time just to pick the cursor shape,
    /// it hit-tests against whatever canonical build is currently stored — the
    /// layout the user is actually looking at (the semantically correct target
    /// for "what am I pointing at"). The redraw-time `cursor_probe` remains the
    /// single hash per cursor move and self-corrects any hint staleness on the
    /// next painted frame.
    ///
    /// When no build exists yet (no paint since launch) the transcript region
    /// yields no hint — the caller falls back to the default arrow, which is
    /// acceptable because the very next paint stores a build. Net on this path:
    /// zero transcript fingerprints.
    pub fn hit_test_current_build(&self, rect: Rect, point: Point2D) -> Option<AIChatHit> {
        if self.state.messages.is_empty() {
            // No transcript region — the non-transcript hits (header / input /
            // overlays) don't consult a canonical build, so this stays hash-free.
            return self.hit_test_with_canonical(rect, point, None);
        }
        crate::widgets::ai_chat_transcript_cache::with_current_canonical(self.owner, |current| {
            match current {
                // Pure geometric hit against the displayed layout — no hashing.
                Some((_, canonical)) => self.hit_test_with_canonical(rect, point, Some(canonical)),
                // Nothing painted yet, or the stored build belongs to a
                // different panel: no hint (arrow) until this panel paints.
                None => None,
            }
        })
    }

    pub fn footer_hover_at(
        &self,
        rect: Rect,
        point: Point2D,
    ) -> Option<de_editor_core::ChatFooterButton> {
        if (self.state.is_minimized() && !self.composer_only) || self.model_picker.open {
            return None;
        }
        if point.y < self.toolbar_top(rect) || !self.composer_card(rect).contains(point) {
            return None;
        }
        let footer = self.footer_layout(rect);
        let streaming = self.is_streaming();
        if footer.plus.contains(point) {
            return Some(de_editor_core::ChatFooterButton::PlusMenu);
        }
        if footer.model.size.x > 0.0 && footer.model.contains(point) {
            return Some(de_editor_core::ChatFooterButton::ModelPicker);
        }
        if streaming && footer.stop.contains(point) {
            return Some(de_editor_core::ChatFooterButton::Stop);
        }
        if footer.send.contains(point) && !self.state.available_models.is_empty() {
            return Some(de_editor_core::ChatFooterButton::Send);
        }
        None
    }

    pub fn example_hover_at(&self, rect: Rect, point: Point2D) -> Option<usize> {
        // Examples are hoverable/clickable regardless of model connection (#43);
        // gate only on messages-empty / not-streaming / not-collapsed.
        if !self.state.messages.is_empty() || self.is_streaming() || self.state.is_minimized() {
            return None;
        }
        // Same visibility predicate as paint: a pill dropped because it
        // would overlap the composer is not hoverable either.
        let region = self.empty_state_region(rect);
        let content_bottom = region.origin.y + region.size.y;
        example_card_rects(rect).iter().position(|card| {
            crate::widgets::ai_chat_panel_paint::example_card_fits(card, content_bottom)
                && (*card).contains(point)
        })
    }
}
