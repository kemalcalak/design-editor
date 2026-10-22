//! The Agents tab's API provider detail (#26): API format, base URL, the
//! masked key, a model checklist fed by the provider's catalog, and a
//! footer with the catalog status, Fetch models and Save. A draft also
//! gets a provider select for the presets the list does not feature.

use crate::theme::Theme;
use crate::widgets::agent_settings_caret;
use crate::widgets::agent_settings_detail_chrome::{
    self as chrome, button_width_estimate, checklist_height, checklist_row_at, paint_button,
    paint_checklist, paint_header, paint_icon_button, paint_input_box, paint_input_value,
    paint_label, paint_segmented, paint_status, segment_at, segmented_width, ButtonKind, CheckRow,
    BUTTON_H, FIELD_GAP, HEADER_H, INPUT_H, INPUT_PAD_X, LABEL_H, VALUE_FONT,
};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{
    AgentSettings, AgentsEntry, BuiltinAgentConfig, BuiltinAgentField, BuiltinAgentKind,
    SettingsFocus,
};
use de_editor_core::agent_settings_builtin_models::{
    BuiltinModelCatalogPhase, BuiltinModelCatalogTarget,
};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::{builtin_agent_preset, BuiltinAgentPresetKey};

const EYE: f32 = 16.0;
const TEXT_LINES: usize = 4;
const FOOTER_GAP: f32 = 28.0;

#[path = "agent_settings_api_preset_menu.rs"]
mod preset_menu;
use preset_menu::{paint_preset_menu, preset_menu_open};
pub(crate) use preset_menu::{preset_at, preset_menu_rect, preset_scroll_max};

/// The provider the pane edits: a saved one by index, or the draft.
#[derive(Clone, Copy)]
pub(crate) struct ApiTarget<'a> {
    pub(crate) agent: &'a BuiltinAgentConfig,
    pub(crate) index: Option<usize>,
}

impl ApiTarget<'_> {
    fn focus(&self, field: BuiltinAgentField) -> SettingsFocus {
        match self.index {
            Some(index) => SettingsFocus::BuiltinAgent { index, field },
            None => SettingsFocus::BuiltinAgentDraft(field),
        }
    }

    fn catalog_target(&self) -> BuiltinModelCatalogTarget {
        match self.index {
            Some(_) => BuiltinModelCatalogTarget::Agent(self.agent.id.clone()),
            None => BuiltinModelCatalogTarget::Draft,
        }
    }
}

/// The API provider `entry` names, if it is one.
pub(crate) fn api_target<'a>(
    settings: &'a AgentSettings,
    entry: &AgentsEntry,
) -> Option<ApiTarget<'a>> {
    match entry {
        AgentsEntry::Builtin(id) => settings
            .builtin_agents
            .iter()
            .enumerate()
            .find(|(_, agent)| agent.id == *id)
            .map(|(index, agent)| ApiTarget {
                agent,
                index: Some(index),
            }),
        AgentsEntry::BuiltinDraft => settings
            .builtin_agent_draft
            .as_ref()
            .map(|agent| ApiTarget { agent, index: None }),
        _ => None,
    }
}

/// Model ids the checklist lists: the saved ones first, then the
/// catalog's other models.
pub(crate) fn model_rows(settings: &AgentSettings, target: ApiTarget<'_>) -> Vec<String> {
    let mut rows: Vec<String> = target
        .agent
        .models
        .iter()
        .filter(|model| !model.trim().is_empty())
        .cloned()
        .collect();
    for option in settings.builtin_model_catalog_options_for(&target.catalog_target()) {
        if !rows.contains(&option.id) {
            rows.push(option.id.clone());
        }
    }
    rows
}

/// How the Models block edits: a checklist of known ids, or a free-text
/// list for endpoints that cannot list models (and while it has focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelsBox {
    Checklist { rows: usize },
    Text,
}

fn models_box(settings: &AgentSettings, target: ApiTarget<'_>) -> ModelsBox {
    let focused = settings.focus == Some(target.focus(BuiltinAgentField::Model));
    let catalog_failed = settings
        .builtin_model_catalog(&target.catalog_target())
        .is_some_and(|catalog| {
            catalog.models.is_empty()
                && matches!(
                    catalog.phase,
                    BuiltinModelCatalogPhase::Error | BuiltinModelCatalogPhase::Unsupported
                )
        });
    let rows = model_rows(settings, target).len();
    if focused || catalog_failed || rows == 0 {
        ModelsBox::Text
    } else {
        ModelsBox::Checklist { rows }
    }
}

/// Every rect of the pane, from the top of `detail`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ApiLayout {
    pub(crate) switch: Option<Rect>,
    pub(crate) format: Option<Rect>,
    pub(crate) provider: Option<Rect>,
    pub(crate) name: Option<Rect>,
    pub(crate) base_url: Option<Rect>,
    pub(crate) api_key: Rect,
    pub(crate) eye: Rect,
    pub(crate) models: Rect,
    pub(crate) models_box: ModelsBox,
    pub(crate) footer: Rect,
    pub(crate) save: Rect,
    pub(crate) fetch: Rect,
    /// Remove for a saved provider, Cancel for the draft.
    pub(crate) secondary: Rect,
    pub(crate) height: f32,
}

fn kind_labels(agent: &BuiltinAgentConfig) -> Vec<(&'static str, BuiltinAgentKind)> {
    let preset = builtin_agent_preset(agent.preset);
    let mut kinds = vec![preset.kind];
    if let Some(alt) = preset.alt_kind {
        kinds.push(alt);
    }
    kinds.sort_by_key(|kind| match kind {
        BuiltinAgentKind::OpenAiCompat => 0,
        BuiltinAgentKind::Anthropic => 1,
    });
    kinds
        .into_iter()
        .map(|kind| match kind {
            BuiltinAgentKind::OpenAiCompat => ("OpenAI-compatible", kind),
            BuiltinAgentKind::Anthropic => ("Anthropic", kind),
        })
        .collect()
}

fn field(x: f32, y: &mut f32, w: f32, h: f32) -> Rect {
    *y += LABEL_H;
    let rect = Rect::xywh(x, *y, w, h);
    *y += h + FIELD_GAP;
    rect
}

pub(crate) fn api_layout(
    detail: Rect,
    settings: &AgentSettings,
    ui: &EditorUiState,
    target: ApiTarget<'_>,
) -> ApiLayout {
    let x = detail.origin.x;
    let w = detail.size.x;
    let mut y = detail.origin.y + HEADER_H + 8.0;
    let agent = target.agent;
    let kinds = kind_labels(agent);
    let format = (kinds.len() > 1).then(|| {
        let labels: Vec<&str> = kinds.iter().map(|(label, _)| *label).collect();
        field(x, &mut y, segmented_width(&labels).min(w), INPUT_H)
    });
    let provider = target.index.is_none().then(|| field(x, &mut y, w, INPUT_H));
    let name = (agent.preset == BuiltinAgentPresetKey::Custom || target.index.is_none())
        .then(|| field(x, &mut y, w, INPUT_H));
    let base_url = agent
        .base_url_editable()
        .then(|| field(x, &mut y, w, INPUT_H));
    let api_key = field(x, &mut y, w, INPUT_H);
    let eye = Rect::xywh(
        api_key.origin.x + api_key.size.x - INPUT_PAD_X - EYE - 4.0,
        api_key.origin.y + (INPUT_H - EYE - 8.0) / 2.0,
        EYE + 8.0,
        EYE + 8.0,
    );
    let models_box = models_box(settings, target);
    let models_h = match models_box {
        ModelsBox::Checklist { rows } => checklist_height(rows),
        ModelsBox::Text => TEXT_LINES as f32 * (VALUE_FONT + 6.0) + 16.0,
    };
    let models = field(x, &mut y, w, models_h);
    let footer = Rect::xywh(x, y - FIELD_GAP + FOOTER_GAP, w, BUTTON_H);
    let right = x + w;
    let save_w = button_width_estimate(save_label(ui, target), false);
    let save = Rect::xywh(right - save_w, footer.origin.y, save_w, BUTTON_H);
    let fetch_w = button_width_estimate(t_settings(ui, "settings.agents.fetchModels"), false);
    let fetch = Rect::xywh(
        save.origin.x - 10.0 - fetch_w,
        footer.origin.y,
        fetch_w,
        BUTTON_H,
    );
    let secondary_w = match target.index {
        Some(_) => BUTTON_H,
        None => button_width_estimate(t_settings(ui, "common.cancel"), false),
    };
    let secondary = Rect::xywh(
        fetch.origin.x - 10.0 - secondary_w,
        footer.origin.y,
        secondary_w,
        BUTTON_H,
    );
    ApiLayout {
        switch: target.index.map(|_| chrome::header_switch_rect(detail)),
        format,
        provider,
        name,
        base_url,
        api_key,
        eye,
        models,
        models_box,
        footer,
        save,
        fetch,
        secondary,
        height: footer.origin.y + BUTTON_H - detail.origin.y,
    }
}

fn save_label<'a>(ui: &'a EditorUiState, target: ApiTarget<'_>) -> &'a str {
    match target.index {
        Some(_) => t_settings(ui, "common.save"),
        None => t_settings(ui, "builtin.add"),
    }
}

/// What a press in the pane does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiHit {
    ToggleEnabled(usize),
    SetKind(BuiltinAgentKind),
    TogglePresetMenu,
    SelectPreset(BuiltinAgentPresetKey),
    Focus(BuiltinAgentField),
    RevealKey,
    ToggleModel(usize),
    Fetch,
    Save,
    Secondary,
    /// Inside the open dropdown but on no row: swallow the press.
    Menu,
    None,
}

pub(crate) fn api_hit(
    settings: &AgentSettings,
    layout: &ApiLayout,
    target: ApiTarget<'_>,
    point: Point2D,
) -> ApiHit {
    if preset_menu_open(settings, target) {
        if let Some(key) = preset_at(settings, layout, point) {
            return ApiHit::SelectPreset(key);
        }
        if preset_menu_rect(layout).is_some_and(|menu| menu.contains(point)) {
            return ApiHit::Menu;
        }
    }
    if let (Some(rect), Some(index)) = (layout.switch, target.index) {
        if rect.contains(point) {
            return ApiHit::ToggleEnabled(index);
        }
    }
    if let Some(rect) = layout.format {
        let kinds = kind_labels(target.agent);
        if let Some(segment) = segment_at(rect, kinds.len(), point) {
            return ApiHit::SetKind(kinds[segment].1);
        }
    }
    if layout.provider.is_some_and(|rect| rect.contains(point)) {
        return ApiHit::TogglePresetMenu;
    }
    if layout.eye.contains(point) {
        return ApiHit::RevealKey;
    }
    for (rect, field) in [
        (layout.name, BuiltinAgentField::DisplayName),
        (layout.base_url, BuiltinAgentField::BaseUrl),
        (Some(layout.api_key), BuiltinAgentField::ApiKey),
    ] {
        if rect.is_some_and(|rect| rect.contains(point)) {
            return ApiHit::Focus(field);
        }
    }
    match layout.models_box {
        ModelsBox::Checklist { rows } => {
            let scroll = settings.builtin_model_menu_scroll.offset;
            if let Some(row) = checklist_row_at(layout.models, rows, scroll, point) {
                return ApiHit::ToggleModel(row);
            }
        }
        ModelsBox::Text if layout.models.contains(point) => {
            return ApiHit::Focus(BuiltinAgentField::Model);
        }
        ModelsBox::Text => {}
    }
    if layout.save.contains(point) {
        return ApiHit::Save;
    }
    if layout.fetch.contains(point) {
        return ApiHit::Fetch;
    }
    if layout.secondary.contains(point) {
        return ApiHit::Secondary;
    }
    ApiHit::None
}

/// The input rect a builtin focus edits, for caret placement.
pub(crate) fn focused_input_rect(layout: &ApiLayout, field: BuiltinAgentField) -> Option<Rect> {
    match field {
        BuiltinAgentField::DisplayName => layout.name,
        BuiltinAgentField::BaseUrl => layout.base_url,
        BuiltinAgentField::ApiKey => Some(layout.api_key),
        BuiltinAgentField::Model => (layout.models_box == ModelsBox::Text).then_some(layout.models),
    }
}

/// `sk-••••••3f9a`: enough of the key to recognise it.
pub(crate) fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len());
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}{}{tail}", "•".repeat((chars.len() - 7).min(18)))
}

fn subtitle(ui: &EditorUiState, agent: &BuiltinAgentConfig) -> String {
    if agent.preset == BuiltinAgentPresetKey::Custom {
        t_settings(ui, "settings.agents.customSubtitle").to_string()
    } else {
        t_settings(ui, "settings.agents.presetSubtitle")
            .replace("{{name}}", builtin_agent_preset(agent.preset).display_name)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_api_detail(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    detail: Rect,
    target: ApiTarget<'_>,
    now_ms: u64,
) {
    let layout = api_layout(detail, settings, ui, target);
    let agent = target.agent;
    let title = if target.index.is_none() && agent.preset == BuiltinAgentPresetKey::Custom {
        t_settings(ui, "settings.agents.addCustomEndpoint").to_string()
    } else {
        agent.display_name.clone()
    };
    paint_header(
        cx,
        theme,
        detail,
        &title,
        &subtitle(ui, agent),
        target.index.map(|_| agent.enabled),
    );
    if let Some(rect) = layout.format {
        let kinds = kind_labels(agent);
        let labels: Vec<&str> = kinds.iter().map(|(label, _)| *label).collect();
        paint_label(cx, theme, t_settings(ui, "settings.agents.apiFormat"), rect);
        let active = kinds.iter().position(|(_, kind)| *kind == agent.kind);
        paint_segmented(cx, theme, rect, &labels, active);
    }
    if let Some(rect) = layout.provider {
        paint_label(cx, theme, t_settings(ui, "builtin.provider"), rect);
        paint_input_box(cx, theme, rect, preset_menu_open(settings, target));
        paint_input_value(
            cx,
            theme,
            rect,
            builtin_agent_preset(agent.preset).display_name,
            "",
            false,
            20.0,
        );
        draw_icon(
            cx.backend,
            Icon::ChevronDown,
            Point2D::new(
                rect.origin.x + rect.size.x - INPUT_PAD_X - 14.0,
                rect.origin.y + (rect.size.y - 14.0) / 2.0,
            ),
            14.0,
            theme.muted_foreground,
            1.7,
        );
    }
    let text_fields = [
        (
            layout.name,
            BuiltinAgentField::DisplayName,
            "builtin.displayName",
            "builtin.displayNamePlaceholder",
            false,
        ),
        (
            layout.base_url,
            BuiltinAgentField::BaseUrl,
            "builtin.baseUrl",
            "builtin.baseUrlPlaceholder",
            true,
        ),
        (
            Some(layout.api_key),
            BuiltinAgentField::ApiKey,
            "builtin.apiKey",
            "settings.agents.apiKeyPlaceholder",
            true,
        ),
    ];
    for (rect, field, label, placeholder, mono) in text_fields {
        let Some(rect) = rect else { continue };
        paint_label(cx, theme, t_settings(ui, label), rect);
        let focused = settings.focus == Some(target.focus(field));
        paint_input_box(cx, theme, rect, focused);
        let is_key = field == BuiltinAgentField::ApiKey;
        let trailing = if is_key { EYE + 12.0 } else { 0.0 };
        if focused {
            let masked = |source: &str| "•".repeat(source.chars().count());
            let mask: Option<&dyn Fn(&str) -> String> =
                (is_key && !settings.api_key_revealed).then_some(&masked);
            agent_settings_caret::paint_settings_input_view_masked(
                cx,
                theme,
                ui,
                Rect::xywh(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.x - trailing,
                    rect.size.y,
                ),
                VALUE_FONT,
                INPUT_PAD_X,
                chrome::centred_baseline(rect, VALUE_FONT),
                now_ms,
                t_settings(ui, placeholder),
                mask,
            );
        } else {
            let value = match field {
                BuiltinAgentField::DisplayName => agent.display_name.clone(),
                BuiltinAgentField::BaseUrl => agent.base_url.clone(),
                BuiltinAgentField::ApiKey if settings.api_key_revealed => agent.api_key.clone(),
                BuiltinAgentField::ApiKey => mask_key(&agent.api_key),
                BuiltinAgentField::Model => String::new(),
            };
            paint_input_value(
                cx,
                theme,
                rect,
                &value,
                t_settings(ui, placeholder),
                mono,
                trailing,
            );
        }
        if is_key {
            draw_icon(
                cx.backend,
                if settings.api_key_revealed {
                    Icon::EyeOff
                } else {
                    Icon::Eye
                },
                Point2D::new(layout.eye.origin.x + 4.0, layout.eye.origin.y + 4.0),
                EYE,
                theme.muted_foreground,
                1.6,
            );
        }
    }
    paint_label(
        cx,
        theme,
        t_settings(ui, "settings.agents.models"),
        layout.models,
    );
    match layout.models_box {
        ModelsBox::Checklist { .. } => {
            let ids = model_rows(settings, target);
            let rows: Vec<CheckRow<'_>> = ids
                .iter()
                .map(|id| CheckRow {
                    label: id,
                    checked: agent.has_model(id),
                })
                .collect();
            paint_checklist(
                cx,
                theme,
                layout.models,
                &rows,
                settings.builtin_model_menu_scroll.offset,
                settings.builtin_model_menu_hover,
            );
        }
        ModelsBox::Text => {
            let focus = target.focus(BuiltinAgentField::Model);
            let focused = settings.focus == Some(focus);
            paint_input_box(cx, theme, layout.models, focused);
            if focused {
                agent_settings_caret::paint_settings_text_area(
                    cx,
                    theme,
                    ui,
                    agent_settings_caret::model_text_area_rect(layout.models),
                    agent_settings_caret::model_text_font_size(),
                    agent_settings_caret::model_text_pad_x(),
                    now_ms,
                    t_settings(ui, "builtin.modelsOnePerLine"),
                    agent_settings_caret::model_text_max_visible_lines(),
                );
            } else {
                let value = agent.models.join(", ");
                let line = Rect::xywh(
                    layout.models.origin.x,
                    layout.models.origin.y,
                    layout.models.size.x,
                    INPUT_H,
                );
                paint_input_value(
                    cx,
                    theme,
                    line,
                    &value,
                    t_settings(ui, "builtin.modelsOnePerLine"),
                    true,
                    0.0,
                );
            }
        }
    }
    paint_footer(cx, theme, settings, ui, &layout, target);
    if preset_menu_open(settings, target) {
        paint_preset_menu(cx, theme, settings, &layout);
    }
}

fn paint_footer(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    layout: &ApiLayout,
    target: ApiTarget<'_>,
) {
    let catalog = settings.builtin_model_catalog(&target.catalog_target());
    let (icon, color, label) = match catalog.map(|catalog| catalog.phase) {
        Some(BuiltinModelCatalogPhase::Loading) => (
            Icon::RefreshCw,
            theme.muted_foreground,
            t_settings(ui, "settings.agents.fetchingModels").to_string(),
        ),
        Some(BuiltinModelCatalogPhase::Ready) => (
            Icon::CheckCircle,
            theme.status_success,
            t_settings(ui, "settings.agents.modelsFound").replace(
                "{{count}}",
                &catalog
                    .map_or(0, |catalog| catalog.models.len())
                    .to_string(),
            ),
        ),
        Some(BuiltinModelCatalogPhase::Error) => (
            Icon::AlertTriangle,
            theme.destructive,
            catalog
                .and_then(|catalog| catalog.error.clone())
                .unwrap_or_else(|| t_settings(ui, "builtin.typeModelManually").to_string()),
        ),
        Some(BuiltinModelCatalogPhase::Unsupported) => (
            Icon::Info,
            theme.muted_foreground,
            t_settings(ui, "builtin.typeModelManually").to_string(),
        ),
        Some(BuiltinModelCatalogPhase::Idle) | None => (
            Icon::Info,
            theme.muted_foreground,
            t_settings(ui, "settings.agents.notCheckedYet").to_string(),
        ),
    };
    let status_w = (layout.secondary.origin.x - 12.0 - layout.footer.origin.x).max(0.0);
    paint_status(
        cx,
        theme,
        layout.footer.origin.x,
        layout.footer,
        icon,
        color,
        &label,
        status_w,
    );
    match target.index {
        Some(_) => paint_icon_button(cx, theme, layout.secondary, Icon::Trash, theme.destructive),
        None => paint_button(
            cx,
            theme,
            layout.secondary,
            t_settings(ui, "common.cancel"),
            None,
            ButtonKind::Outline,
        ),
    }
    paint_button(
        cx,
        theme,
        layout.fetch,
        t_settings(ui, "settings.agents.fetchModels"),
        None,
        ButtonKind::Outline,
    );
    paint_button(
        cx,
        theme,
        layout.save,
        save_label(ui, target),
        None,
        ButtonKind::Primary,
    );
}

#[cfg(test)]
#[path = "agent_settings_api_detail_tests.rs"]
mod tests;
