//! Model-picker dropdown for the AI chat panel — the upward
//! popover that lists saved models grouped by provider.
//! Mirrors the TS `ai-chat-model-selector.tsx` `ModelDropdown`
//! (search row + grouped rows + selected check / qualifiers),
//! restyled to the Home v2 language: a 12 px-radius card on the Home
//! palette, rows with an 18 px provider mark and the model name in the
//! mono face, a filled selected pill, and an "Add a model provider"
//! footer action.
//!
//! This file owns the layout, scroll and hit-test contract; the
//! painting lives in the sibling `ai_chat_model_picker_paint.rs`,
//! declared below with `#[path]` so `widgets/mod.rs` keeps a
//! single picker entry.

use crate::widgets::brand_icons::{
    paint_brand_logo, paint_deepseek_harness_logo, paint_opencode_logo, BrandLogo,
};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use de_editor_core::chat::{AgentProvider, ModelEntry};
pub use jian_widgets::components::select::SelectHit;
use jian_widgets::components::select::SelectState;

#[path = "ai_chat_model_picker_paint.rs"]
mod paint;

pub use paint::paint_model_picker;
// Layout helper surfaced for the row-text collision tests.
#[cfg(test)]
pub(crate) use paint::fit_row_text;

/// Height of a provider group-header row.
pub const MODEL_GROUP_H: f32 = 26.0;
/// Height of a single model row.
pub const MODEL_ROW_H: f32 = 36.0;
/// Vertical padding inside the dropdown card's list area (top +
/// bottom each).
pub const MODEL_PICKER_PAD_Y: f32 = 8.0;
/// Fixed search strip at the top of the dropdown.
pub const MODEL_SEARCH_H: f32 = 40.0;
/// Fixed "添加模型" action row closing the card under the list.
pub const MODEL_FOOTER_H: f32 = 36.0;
/// Hard cap on the dropdown's painted height. A connected catalog
/// taller than this (e.g. OpenCode's 75+ models) scrolls inside the
/// card instead of growing off the top of the screen.
pub const MODEL_PICKER_MAX_H: f32 = 288.0;
const MODEL_EMPTY_H: f32 = 44.0;

/// Painted height of the dropdown for `models` — the content height
/// clamped to [`MODEL_PICKER_MAX_H`].
pub fn picker_view_height(models: &[ModelEntry], offline: &[AgentProvider], search: &str) -> f32 {
    picker_content_height(models, offline, search).min(MODEL_PICKER_MAX_H)
}

/// Largest valid scroll offset for `models` — `0` when the content
/// already fits inside [`MODEL_PICKER_MAX_H`]. The search strip and
/// the footer are fixed chrome, so only the list band between them
/// scrolls.
pub fn max_picker_scroll(models: &[ModelEntry], offline: &[AgentProvider], search: &str) -> f32 {
    let view_list_h =
        (picker_view_height(models, offline, search) - MODEL_SEARCH_H - MODEL_FOOTER_H).max(0.0);
    (picker_list_height(models, offline, search) - view_list_h).max(0.0)
}

/// CLI agents installed on this machine (their models were discovered)
/// that the user has not connected: the picker lists them under the
/// connected groups with a Connect action (#26).
pub fn offline_agents(state: &de_editor_core::EditorState) -> Vec<AgentProvider> {
    let connected = &state.editor_ui.agent_settings.connected;
    AgentProvider::DISPLAY
        .into_iter()
        .filter(|provider| !connected[provider.index()])
        .filter(|provider| {
            state
                .chat
                .discovered_models
                .iter()
                .any(|m| m.provider == *provider && !is_builtin(m) && !is_acp(m))
        })
        .collect()
}

fn visible_offline(offline: &[AgentProvider], search: &str) -> Vec<AgentProvider> {
    let q = normalized_query(search);
    offline
        .iter()
        .copied()
        .filter(|p| q.is_empty() || p.name().to_lowercase().contains(&q))
        .collect()
}

/// Where a model comes from, in the order the picker lists the groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GroupKind {
    Api,
    Cli,
    Acp,
}

impl GroupKind {
    fn of(entry: &ModelEntry) -> Self {
        if is_builtin(entry) {
            GroupKind::Api
        } else if is_acp(entry) {
            GroupKind::Acp
        } else {
            GroupKind::Cli
        }
    }

    fn tag(self) -> &'static str {
        match self {
            GroupKind::Api => "API",
            GroupKind::Cli => "CLI",
            GroupKind::Acp => "ACP",
        }
    }
}

/// The tag a group header carries at its right edge.
pub(crate) fn group_tag(entry: &ModelEntry) -> &'static str {
    GroupKind::of(entry).tag()
}

/// One laid-out row in the dropdown.
enum Row {
    /// Provider group header — a typographic label; the provider's
    /// mark is painted inside each model row instead.
    Header {
        /// `false` for every group after the first — those get a
        /// hairline above them.
        first_group: bool,
        label: String,
        /// "API" / "CLI" / "ACP".
        tag: &'static str,
    },
    /// Selectable model — carries its index into the flat list.
    Model { idx: usize },
    /// An installed agent that is not connected yet, with Connect.
    Offline { provider: AgentProvider },
}

fn normalized_query(search: &str) -> String {
    search.trim().to_lowercase()
}

fn is_builtin(entry: &ModelEntry) -> bool {
    entry.builtin_provider_id.is_some() || entry.value.starts_with("builtin:")
}

fn is_acp(entry: &ModelEntry) -> bool {
    entry.acp_agent_id().is_some()
}

fn same_group(a: &ModelEntry, b: &ModelEntry) -> bool {
    if is_acp(a) || is_acp(b) {
        return is_acp(a) && is_acp(b) && a.value == b.value;
    }
    a.provider == b.provider && a.builtin_provider_id == b.builtin_provider_id
}

fn model_matches(entry: &ModelEntry, q: &str) -> bool {
    q.is_empty()
        || entry.display_name.to_lowercase().contains(q)
        || entry.value.to_lowercase().contains(q)
        || searchable_group_label(entry).is_some_and(|label| label.to_lowercase().contains(q))
}

fn searchable_group_label(entry: &ModelEntry) -> Option<String> {
    if is_acp(entry) {
        return Some(group_label_for_entry(entry));
    }
    if is_builtin(entry) {
        return entry
            .builtin_provider_display_name
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_string)
            .or_else(|| {
                (entry.provider == AgentProvider::ClaudeCode)
                    .then(|| "Anthropic (API Key)".to_string())
            });
    }
    Some(entry.provider.name().to_string())
}

pub fn visible_model_indices(models: &[ModelEntry], search: &str) -> Vec<usize> {
    let q = normalized_query(search);
    models
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| model_matches(entry, &q).then_some(idx))
        .collect()
}

pub(crate) fn grouped_visible_model_indices(
    models: &[ModelEntry],
    search: &str,
) -> Vec<Vec<usize>> {
    let visible = visible_model_indices(models, search);
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for idx in visible {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| same_group(&models[group[0]], &models[idx]))
        {
            group.push(idx);
        } else {
            groups.push(vec![idx]);
        }
    }
    // API providers first, then agents on this machine, then ACP agents;
    // first-seen order within each.
    groups.sort_by_key(|group| GroupKind::of(&models[group[0]]));
    groups
}

/// Walk the dropdown row layout, invoking `f(row, y, height)` for
/// each row top-to-bottom starting at `top`. Paint and hit-test
/// both drive off this so they never drift apart.
fn walk_rows(
    models: &[ModelEntry],
    offline: &[AgentProvider],
    search: &str,
    top: f32,
    mut f: impl FnMut(&Row, f32, f32),
) {
    let mut y = top + MODEL_PICKER_PAD_Y;
    for (group_idx, group) in grouped_visible_model_indices(models, search)
        .into_iter()
        .enumerate()
    {
        let first_idx = group[0];
        let entry = &models[first_idx];
        f(
            &Row::Header {
                first_group: group_idx == 0,
                label: group_label_for_entry(entry),
                tag: group_tag(entry),
            },
            y,
            MODEL_GROUP_H,
        );
        y += MODEL_GROUP_H;
        for idx in group {
            f(&Row::Model { idx }, y, MODEL_ROW_H);
            y += MODEL_ROW_H;
        }
    }
    for provider in visible_offline(offline, search) {
        f(&Row::Offline { provider }, y, MODEL_ROW_H);
        y += MODEL_ROW_H;
    }
}

/// Total dropdown height for `models` (search strip + group headers
/// + rows + the top/bottom list padding + the footer action row).
pub fn picker_content_height(
    models: &[ModelEntry],
    offline: &[AgentProvider],
    search: &str,
) -> f32 {
    MODEL_SEARCH_H + picker_list_height(models, offline, search) + MODEL_FOOTER_H
}

fn picker_list_height(models: &[ModelEntry], offline: &[AgentProvider], search: &str) -> f32 {
    let groups = grouped_visible_model_indices(models, search);
    let offline_count = visible_offline(offline, search).len();
    if groups.is_empty() && offline_count == 0 {
        return MODEL_EMPTY_H;
    }
    let model_count: usize = groups.iter().map(Vec::len).sum();
    groups.len() as f32 * MODEL_GROUP_H
        + (model_count + offline_count) as f32 * MODEL_ROW_H
        + MODEL_PICKER_PAD_Y * 2.0
}

/// Whether the list has anything to show (models or offline agents).
fn has_rows(models: &[ModelEntry], offline: &[AgentProvider], search: &str) -> bool {
    !visible_model_indices(models, search).is_empty()
        || !visible_offline(offline, search).is_empty()
}

/// Map a click inside the dropdown `rect` to the index of the
/// model row under it. `None` for a click on a header / footer /
/// padding. `scroll` is the dropdown's vertical scroll offset in
/// px — paint and hit-test share it so a scrolled row resolves
/// correctly.
pub fn model_at(
    rect: Rect,
    point: Point2D,
    models: &[ModelEntry],
    offline: &[AgentProvider],
    scroll: f32,
    search: &str,
) -> Option<usize> {
    let state = SelectState {
        open: true,
        scroll: jian_core::scroll::ScrollState { offset: scroll },
        ..Default::default()
    };
    match model_picker_hit(&state, rect, point, models, offline, search) {
        SelectHit::Row(index) => Some(index),
        SelectHit::Inside | SelectHit::Outside => None,
    }
}

/// Shared select-style hit protocol for the searchable model picker.
/// Search/header/footer/empty/padding chrome returns `Inside`; model
/// rows return `Row(index)` where `index` addresses `available_models`.
pub fn model_picker_hit(
    state: &SelectState,
    rect: Rect,
    point: Point2D,
    models: &[ModelEntry],
    offline: &[AgentProvider],
    search: &str,
) -> SelectHit {
    if !state.open {
        return SelectHit::Outside;
    }
    if point.x < rect.origin.x
        || point.x > rect.origin.x + rect.size.x
        || point.y < rect.origin.y
        || point.y > rect.origin.y + rect.size.y
    {
        return SelectHit::Outside;
    }
    let list_rect = model_list_rect(rect);
    if point.y < list_rect.origin.y {
        return SelectHit::Inside;
    }
    let mut hit = SelectHit::Inside;
    // Walk from a scroll-shifted origin — the same offset paint
    // applies via `translate` — then keep only hits whose row band
    // intersects the visible list band (paint clips rows there, so
    // the footer row below the list can never resolve to a model).
    let list_bottom = list_rect.origin.y + list_rect.size.y;
    walk_rows(
        models,
        offline,
        search,
        list_rect.origin.y - state.scroll.offset,
        |row, y, h| {
            if point.y >= y
                && point.y < y + h
                && point.y >= list_rect.origin.y
                && point.y < list_bottom
            {
                hit = match row {
                    Row::Model { idx, .. } => SelectHit::Row(*idx),
                    Row::Header { .. } | Row::Offline { .. } => SelectHit::Inside,
                };
            }
        },
    );
    hit
}

/// The offline agent whose row (and so whose Connect) is under `point`.
pub fn offline_agent_at(
    state: &SelectState,
    rect: Rect,
    point: Point2D,
    models: &[ModelEntry],
    offline: &[AgentProvider],
    search: &str,
) -> Option<AgentProvider> {
    if !state.open || !rect.contains(point) {
        return None;
    }
    let list_rect = model_list_rect(rect);
    if !list_rect.contains(point) {
        return None;
    }
    let mut found = None;
    walk_rows(
        models,
        offline,
        search,
        list_rect.origin.y - state.scroll.offset,
        |row, y, h| {
            if let Row::Offline { provider } = row {
                if point.y >= y && point.y < y + h {
                    found = Some(*provider);
                }
            }
        },
    );
    found
}

/// The footer row ("Manage providers & agents…").
pub fn footer_rect(rect: Rect) -> Rect {
    Rect::xywh(
        rect.origin.x,
        rect.origin.y + rect.size.y - MODEL_FOOTER_H,
        rect.size.x,
        MODEL_FOOTER_H,
    )
}

pub fn search_clear_hit(rect: Rect, point: Point2D, search: &str) -> bool {
    !search.is_empty() && search_clear_rect(rect).contains(point)
}

/// The scrollable list band between the fixed search strip and the
/// fixed footer row.
fn model_list_rect(rect: Rect) -> Rect {
    Rect {
        origin: Point2D::new(rect.origin.x, rect.origin.y + MODEL_SEARCH_H),
        size: Point2D::new(
            rect.size.x,
            (rect.size.y - MODEL_SEARCH_H - MODEL_FOOTER_H).max(0.0),
        ),
    }
}

/// The 32 px rounded search well floating in the 40 px search strip.
fn search_field_rect(rect: Rect) -> Rect {
    Rect {
        origin: Point2D::new(rect.origin.x + 10.0, rect.origin.y + 4.0),
        size: Point2D::new((rect.size.x - 20.0).max(0.0), 32.0),
    }
}

fn search_clear_rect(rect: Rect) -> Rect {
    let search = search_field_rect(rect);
    Rect {
        origin: Point2D::new(search.origin.x + search.size.x - 28.0, search.origin.y),
        size: Point2D::new(28.0, search.size.y),
    }
}

/// The filled pill a selected / hovered / pressed model row paints:
/// inset 6 px from the popover's left and right edges and 3 px from
/// the row's top and bottom, radius 8. Shared by paint and the
/// geometry tests.
pub(crate) fn model_row_pill_rect(rect: Rect, row_y: f32) -> Rect {
    Rect {
        origin: Point2D::new(rect.origin.x + 6.0, row_y + 3.0),
        size: Point2D::new((rect.size.x - 12.0).max(0.0), MODEL_ROW_H - 6.0),
    }
}

/// Paint a provider's brand logo into a `size × size` square.
/// OpenCode has no single-path logo, so it routes through the
/// multi-primitive `paint_opencode_logo`.
pub fn paint_provider_logo(
    cx: &mut PaintCx<'_>,
    provider: AgentProvider,
    top_left: Point2D,
    size: f32,
    color: Color,
) {
    match provider {
        AgentProvider::ClaudeCode => {
            paint_brand_logo(cx.backend, BrandLogo::Claude, top_left, size, color)
        }
        AgentProvider::CodexCli => {
            paint_brand_logo(cx.backend, BrandLogo::OpenAI, top_left, size, color)
        }
        AgentProvider::GithubCopilot => {
            paint_brand_logo(cx.backend, BrandLogo::Copilot, top_left, size, color)
        }
        AgentProvider::OpenCode => paint_opencode_logo(cx.backend, top_left, size, color),
        AgentProvider::Antigravity => {
            paint_brand_logo(cx.backend, BrandLogo::Antigravity, top_left, size, color)
        }
        AgentProvider::GrokBuild => {
            paint_brand_logo(cx.backend, BrandLogo::Grok, top_left, size, color)
        }
        AgentProvider::DeepSeekHarness => {
            paint_deepseek_harness_logo(cx.backend, top_left, size, color)
        }
    }
}

/// Uppercase provider name for the group header (matches the TS
/// dropdown's `providerName` styling).
pub(super) fn provider_label(provider: AgentProvider) -> &'static str {
    match provider {
        AgentProvider::ClaudeCode => "ANTHROPIC",
        AgentProvider::CodexCli => "OPENAI",
        AgentProvider::GithubCopilot => "GITHUB COPILOT",
        AgentProvider::OpenCode => "OPENCODE",
        AgentProvider::Antigravity => "ANTIGRAVITY",
        AgentProvider::GrokBuild => "GROK BUILD",
        AgentProvider::DeepSeekHarness => "DEEPSEEK HARNESS",
    }
}

fn group_label(provider: AgentProvider, builtin: bool) -> &'static str {
    if builtin {
        match provider {
            AgentProvider::ClaudeCode => "ANTHROPIC API KEY",
            AgentProvider::CodexCli => "OPENAI API KEY",
            AgentProvider::GithubCopilot => "COPILOT API KEY",
            AgentProvider::OpenCode => "OPENCODE API KEY",
            AgentProvider::Antigravity => "ANTIGRAVITY API KEY",
            AgentProvider::GrokBuild => "GROK BUILD API KEY",
            AgentProvider::DeepSeekHarness => "DEEPSEEK API KEY",
        }
    } else {
        provider_label(provider)
    }
}

pub(super) fn group_label_for_entry(entry: &ModelEntry) -> String {
    if is_acp(entry) {
        let name = entry.display_name.trim();
        if name.is_empty() {
            return "ACP".to_string();
        }
        return format!("{name} (ACP)").to_uppercase();
    }
    if is_builtin(entry) {
        if let Some(label) = entry
            .builtin_provider_display_name
            .as_deref()
            .map(str::trim)
            .filter(|label| !label.is_empty())
        {
            return label.to_uppercase();
        }
    }
    if is_builtin(entry) {
        return group_label(entry.provider, true).to_string();
    }
    // An agent CLI on this machine reads by its own name (#26).
    entry.provider.name().to_uppercase()
}
