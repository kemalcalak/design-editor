//! The composer's model slot: the "<model> · <provider>" button, the
//! "Add a model provider →" link that replaces it when nothing can
//! answer, and the geometry of the chat model picker opened above it.

use super::copy::tr;
use super::layout::MODEL_FONT;
use super::paint::{draw_text, draw_text_weighted};
use super::{HomeLayout, HomeSurface, SANS};
use crate::widgets::ai_chat_model_picker::picker_view_height;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::{EditorState, HomeHit, ModelEntry};

/// Width of the Home-anchored model picker card.
pub const HOME_MODEL_PICKER_W: f32 = 300.0;
/// The card floats this far above the button.
pub const HOME_MODEL_PICKER_GAP: f32 = 8.0;

/// The label the model button shows: the selected model and the
/// provider that serves it. Empty when no agent can answer (the slot
/// shows the provider link instead).
pub fn model_chip_label(state: &EditorState) -> String {
    if !state.has_usable_chat_agent() {
        return String::new();
    }
    let Some(entry) = state.chat.selected_model_entry() else {
        return String::new();
    };
    let provider = entry
        .builtin_provider_display_name
        .as_deref()
        .unwrap_or_else(|| entry.provider.name());
    format!("{} · {provider}", entry.display_name)
}

/// The picker card anchored above the model button. `None` when the
/// button is not laid out.
pub fn home_model_picker_rects(
    layout: &HomeLayout,
    viewport_w: f32,
    models: &[ModelEntry],
    offline: &[de_editor_core::chat::AgentProvider],
    search: &str,
) -> Option<Rect> {
    let chip = layout.model_chip;
    if chip.size.x <= 0.0 {
        return None;
    }
    let height = picker_view_height(models, offline, search);
    let bottom = chip.origin.y - HOME_MODEL_PICKER_GAP;
    let top = (bottom - height).max(8.0);
    let x = (chip.origin.x + chip.size.x - HOME_MODEL_PICKER_W)
        .clamp(8.0, (viewport_w - HOME_MODEL_PICKER_W - 8.0).max(8.0));
    Some(Rect::xywh(x, top, HOME_MODEL_PICKER_W, height))
}

/// Paint the model slot: the quiet label + chevron, or the accent
/// provider link + arrow.
pub(super) fn paint_model_slot(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, rect: Rect) {
    let p = surface.palette;
    let baseline = jian_widgets::centered_text_baseline_y(rect, MODEL_FONT);
    if !surface.usable_agent {
        let label = tr(surface.locale, "home.model.addProvider");
        let hovered = surface.state.hover == Some(HomeHit::AddProvider);
        draw_text_weighted(
            cx,
            label,
            SANS,
            MODEL_FONT,
            if hovered { p.text } else { p.accent },
            500,
            Point2D::new(rect.origin.x + 4.0, baseline),
        );
        draw_icon(
            cx.backend,
            Icon::ArrowRight,
            Point2D::new(
                rect.origin.x + rect.size.x - 18.0,
                rect.origin.y + (rect.size.y - 12.0) / 2.0,
            ),
            12.0,
            p.accent,
            1.8,
        );
        return;
    }
    let hovered = matches!(surface.state.hover, Some(HomeHit::ModelChip));
    if hovered {
        cx.backend.fill_round_rect(rect, 6.0, p.subtle);
    }
    cx.backend.save();
    cx.backend.clip_rect(Rect::xywh(
        rect.origin.x,
        rect.origin.y,
        rect.size.x - 20.0,
        rect.size.y,
    ));
    draw_text(
        cx,
        &surface.chip_label,
        SANS,
        MODEL_FONT,
        if hovered { p.text } else { p.text_2 },
        Point2D::new(rect.origin.x + 8.0, baseline),
    );
    cx.backend.restore();
    draw_icon(
        cx.backend,
        Icon::ChevronDown,
        Point2D::new(
            rect.origin.x + rect.size.x - 18.0,
            rect.origin.y + (rect.size.y - 12.0) / 2.0,
        ),
        12.0,
        p.text_3,
        1.6,
    );
}
