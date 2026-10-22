//! The Agents tab body (#26): the list on the left and the selected
//! entry's detail on the right, under the tab intro. Paint, hit-test,
//! hover and the caret's input rects all route through here so the three
//! detail panes stay behind one seam.

use crate::theme::Theme;
use crate::widgets::agent_settings_acp_detail::{self as acp, AcpHit};
use crate::widgets::agent_settings_agents_list::{self as list, ListItem};
use crate::widgets::agent_settings_api_detail::{self as api, ApiHit};
use crate::widgets::agent_settings_cli_detail::{self as cli, CliHit};
use crate::widgets::agent_settings_detail_chrome::{checklist_row_at, checklist_scroll_max};
use crate::widgets::agent_settings_panel::AgentSettingsHit;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::{AgentSettings, AgentsEntry, SettingsFocus};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::{BuiltinAgentPresetKey, ModelEntry};

/// What the tab reads for one frame.
#[derive(Clone, Copy)]
pub(crate) struct AgentsTab<'a> {
    pub(crate) settings: &'a AgentSettings,
    pub(crate) ui: &'a EditorUiState,
    pub(crate) discovered: &'a [ModelEntry],
    /// Desktop lists CLI and ACP agents; the web build only API providers.
    pub(crate) external: bool,
}

/// The detail pane's content for the selected entry.
enum Detail<'a> {
    Api(api::ApiTarget<'a>),
    Cli(de_editor_core::agent_settings::AgentProvider),
    Acp(acp::AcpTarget<'a>),
}

impl<'a> AgentsTab<'a> {
    fn detail(&self) -> Option<Detail<'a>> {
        let entry = self.settings.resolved_agents_entry(self.external)?;
        match &entry {
            AgentsEntry::Builtin(_) | AgentsEntry::BuiltinDraft => {
                api::api_target(self.settings, &entry).map(Detail::Api)
            }
            AgentsEntry::Cli(provider) => Some(Detail::Cli(*provider)),
            AgentsEntry::Acp(_) | AgentsEntry::AcpDraft => {
                acp::acp_target(self.settings, &entry).map(Detail::Acp)
            }
        }
    }

    /// The list and detail columns inside `body`.
    pub(crate) fn columns(&self, body: Rect) -> (Rect, Rect) {
        list::columns(body)
    }

    fn detail_height(&self, detail_rect: Rect) -> f32 {
        match self.detail() {
            Some(Detail::Api(target)) => {
                api::api_layout(detail_rect, self.settings, self.ui, target).height
            }
            Some(Detail::Cli(provider)) => {
                cli::cli_layout(
                    detail_rect,
                    self.settings,
                    self.ui,
                    self.discovered,
                    provider,
                )
                .height
            }
            Some(Detail::Acp(target)) => {
                acp::acp_layout(detail_rect, self.settings, self.ui, target).height
            }
            None => 0.0,
        }
    }

    /// Height of the body below the intro.
    pub(crate) fn body_height(&self, body: Rect) -> f32 {
        let (_, detail) = self.columns(body);
        list::list_height(self.settings, self.external).max(self.detail_height(detail))
    }

    pub(crate) fn paint(&self, cx: &mut PaintCx<'_>, theme: &Theme, body: Rect, now_ms: u64) {
        let (list_rect, detail) = self.columns(body);
        list::paint_list(
            cx,
            theme,
            self.ui,
            self.settings,
            list_rect,
            self.external,
            self.settings.hover_agents_row,
        );
        let height = self.body_height(body).max(body.size.y);
        cx.backend.fill_rect(
            Rect::xywh(list::divider_x(body), body.origin.y, 1.0, height),
            theme.border,
        );
        match self.detail() {
            Some(Detail::Api(target)) => {
                api::paint_api_detail(cx, theme, self.settings, self.ui, detail, target, now_ms)
            }
            Some(Detail::Cli(provider)) => cli::paint_cli_detail(
                cx,
                theme,
                self.settings,
                self.ui,
                detail,
                self.discovered,
                provider,
            ),
            Some(Detail::Acp(target)) => {
                acp::paint_acp_detail(cx, theme, self.settings, self.ui, detail, target, now_ms)
            }
            None => {}
        }
    }

    /// The press under `point` (already in scrolled body space).
    pub(crate) fn hit(&self, body: Rect, point: Point2D) -> AgentSettingsHit {
        let (list_rect, detail) = self.columns(body);
        // The provider dropdown overlays the fields under it, so it wins.
        if let Some(Detail::Api(target)) = self.detail() {
            let layout = api::api_layout(detail, self.settings, self.ui, target);
            if let Some(hit) =
                self.api_hit(api::api_hit(self.settings, &layout, target, point), target)
            {
                return hit;
            }
        }
        if let Some((_, item)) = list::row_at(list_rect, self.settings, self.external, point) {
            return match item {
                ListItem::Builtin(index) => AgentSettingsHit::SelectBuiltinEntry(index),
                ListItem::Preset(key) => AgentSettingsHit::SelectApiPreset(key),
                ListItem::Cli(provider) => AgentSettingsHit::SelectCliEntry(provider),
                ListItem::Acp(index) => AgentSettingsHit::SelectAcpEntry(index),
                ListItem::AddCustom => AgentSettingsHit::AddProvider,
                ListItem::AddAcp => AgentSettingsHit::AddAcpAgent,
                ListItem::Section(_) => AgentSettingsHit::Inside,
            };
        }
        match self.detail() {
            Some(Detail::Cli(provider)) => {
                let layout =
                    cli::cli_layout(detail, self.settings, self.ui, self.discovered, provider);
                match cli::cli_hit(self.settings, &layout, point) {
                    CliHit::Recheck => AgentSettingsHit::RecheckProvider(provider),
                    CliHit::ToggleModel(row) => AgentSettingsHit::ToggleCliModel { provider, row },
                    CliHit::Connect => AgentSettingsHit::Connect(provider),
                    CliHit::None => AgentSettingsHit::Inside,
                }
            }
            Some(Detail::Acp(target)) => {
                let layout = acp::acp_layout(detail, self.settings, self.ui, target);
                match (acp::acp_hit(&layout, point), target.index) {
                    (AcpHit::SelectPreset(row), _) => AgentSettingsHit::SelectAcpPreset(row),
                    (AcpHit::SetConnection(kind), index) => {
                        AgentSettingsHit::SetAcpConnection { index, kind }
                    }
                    (AcpHit::Focus(field), Some(index)) => {
                        AgentSettingsHit::FocusAcpAgent { index, field }
                    }
                    (AcpHit::Focus(field), None) => AgentSettingsHit::FocusAcpAgentDraft(field),
                    (AcpHit::Primary, Some(index)) => AgentSettingsHit::ToggleAcpConnected(index),
                    (AcpHit::Primary, None) => AgentSettingsHit::SaveAcpAgentDraft,
                    (AcpHit::Secondary, Some(index)) => AgentSettingsHit::RemoveAcpAgent(index),
                    (AcpHit::Secondary, None) => AgentSettingsHit::CancelAcpAgentDraft,
                    (AcpHit::None, _) => AgentSettingsHit::Inside,
                }
            }
            _ => AgentSettingsHit::Inside,
        }
    }

    /// `Some` when the API pane claims the press.
    fn api_hit(&self, hit: ApiHit, target: api::ApiTarget<'_>) -> Option<AgentSettingsHit> {
        let index = target.index;
        Some(match hit {
            ApiHit::ToggleEnabled(index) => AgentSettingsHit::ToggleBuiltinAgentEnabled(index),
            ApiHit::SetKind(kind) if kind == target.agent.kind => AgentSettingsHit::Inside,
            ApiHit::SetKind(_) => match index {
                Some(index) => AgentSettingsHit::ToggleBuiltinAgentKind(index),
                None => AgentSettingsHit::ToggleBuiltinAgentDraftKind,
            },
            ApiHit::TogglePresetMenu => AgentSettingsHit::ToggleBuiltinAgentPresetMenu(index),
            ApiHit::SelectPreset(preset) => {
                AgentSettingsHit::SelectBuiltinAgentPreset { index, preset }
            }
            ApiHit::Focus(field) => match index {
                Some(index) => AgentSettingsHit::FocusBuiltinAgent { index, field },
                None => AgentSettingsHit::FocusBuiltinAgentDraft(field),
            },
            ApiHit::RevealKey => AgentSettingsHit::ToggleApiKeyReveal,
            ApiHit::ToggleModel(row) => AgentSettingsHit::SelectBuiltinModel { index, row },
            ApiHit::Fetch => AgentSettingsHit::FetchBuiltinModels(index),
            ApiHit::Save => match index {
                Some(index) => AgentSettingsHit::SaveBuiltinAgentEditing(index),
                None => AgentSettingsHit::SaveBuiltinAgentDraft,
            },
            ApiHit::Secondary => match index {
                Some(index) => AgentSettingsHit::RemoveBuiltinAgent(index),
                None => AgentSettingsHit::CancelBuiltinAgentDraft,
            },
            ApiHit::Menu => AgentSettingsHit::Inside,
            ApiHit::None => return None,
        })
    }

    /// The list line under `point`, for hover.
    pub(crate) fn row_at(&self, body: Rect, point: Point2D) -> Option<usize> {
        let (list_rect, _) = self.columns(body);
        list::row_at(list_rect, self.settings, self.external, point).map(|(index, _)| index)
    }

    /// The detail's checklist box and its row count, if it shows one.
    fn checklist(&self, body: Rect) -> Option<(Rect, usize)> {
        let (_, detail) = self.columns(body);
        match self.detail()? {
            Detail::Api(target) => {
                let layout = api::api_layout(detail, self.settings, self.ui, target);
                match layout.models_box {
                    api::ModelsBox::Checklist { rows } => Some((layout.models, rows)),
                    api::ModelsBox::Text => None,
                }
            }
            Detail::Cli(provider) => {
                let layout =
                    cli::cli_layout(detail, self.settings, self.ui, self.discovered, provider);
                (layout.model_rows > 0).then_some((layout.models, layout.model_rows))
            }
            Detail::Acp(_) => None,
        }
    }

    /// The checklist row under `point`, for hover.
    pub(crate) fn model_row_at(&self, body: Rect, point: Point2D) -> Option<usize> {
        let (boxed, rows) = self.checklist(body)?;
        checklist_row_at(
            boxed,
            rows,
            self.settings.builtin_model_menu_scroll.offset,
            point,
        )
    }

    /// Wheel cap for the checklist under `point`.
    pub(crate) fn model_scroll_max_at(&self, body: Rect, point: Point2D) -> Option<f32> {
        let (boxed, rows) = self.checklist(body)?;
        boxed.contains(point).then(|| checklist_scroll_max(rows))
    }

    fn api_layout(&self, body: Rect) -> Option<(api::ApiLayout, api::ApiTarget<'a>)> {
        let (_, detail) = self.columns(body);
        match self.detail()? {
            Detail::Api(target) => Some((
                api::api_layout(detail, self.settings, self.ui, target),
                target,
            )),
            _ => None,
        }
    }

    /// The provider-dropdown row under `point`, for hover.
    pub(crate) fn preset_hover_at(
        &self,
        body: Rect,
        point: Point2D,
    ) -> Option<BuiltinAgentPresetKey> {
        let (layout, target) = self.api_layout(body)?;
        if target.index.is_some() || self.settings.builtin_preset_menu_open.is_none() {
            return None;
        }
        api::preset_at(self.settings, &layout, point)
    }

    /// Wheel cap for the open provider dropdown under `point`.
    pub(crate) fn preset_scroll_max_at(&self, body: Rect, point: Point2D) -> Option<f32> {
        let (layout, target) = self.api_layout(body)?;
        if target.index.is_some() || self.settings.builtin_preset_menu_open.is_none() {
            return None;
        }
        api::preset_menu_rect(&layout)
            .filter(|menu| menu.contains(point))
            .map(|_| api::preset_scroll_max())
    }

    /// The input the focused field edits, in unscrolled body space.
    pub(crate) fn focused_input_rect(&self, body: Rect) -> Option<Rect> {
        let focus = self.settings.focus?;
        let (_, detail) = self.columns(body);
        match (self.detail()?, focus) {
            (Detail::Api(target), SettingsFocus::BuiltinAgent { index, field })
                if target.index == Some(index) =>
            {
                api::focused_input_rect(
                    &api::api_layout(detail, self.settings, self.ui, target),
                    field,
                )
            }
            (Detail::Api(target), SettingsFocus::BuiltinAgentDraft(field))
                if target.index.is_none() =>
            {
                api::focused_input_rect(
                    &api::api_layout(detail, self.settings, self.ui, target),
                    field,
                )
            }
            (Detail::Acp(target), SettingsFocus::AcpAgent { index, field })
                if target.index == Some(index) =>
            {
                acp::focused_input_rect(
                    &acp::acp_layout(detail, self.settings, self.ui, target),
                    field,
                )
            }
            (Detail::Acp(target), SettingsFocus::AcpAgentDraft(field))
                if target.index.is_none() =>
            {
                acp::focused_input_rect(
                    &acp::acp_layout(detail, self.settings, self.ui, target),
                    field,
                )
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_core::agent_settings::AgentProvider;

    fn body() -> Rect {
        Rect::xywh(228.0, 100.0, 704.0, 600.0)
    }

    fn tab<'a>(
        settings: &'a AgentSettings,
        ui: &'a EditorUiState,
        external: bool,
    ) -> AgentsTab<'a> {
        AgentsTab {
            settings,
            ui,
            discovered: &[],
            external,
        }
    }

    #[test]
    fn list_rows_select_their_entry() {
        let settings = AgentSettings::default();
        let ui = EditorUiState::default();
        let tab = tab(&settings, &ui, true);
        let (list_rect, _) = tab.columns(body());
        let rows = list::list_rows(list_rect, &settings, true);
        let centre = |item: ListItem| {
            let rect = rows.iter().find(|(row, _)| *row == item).unwrap().1;
            Point2D::new(rect.origin.x + 20.0, rect.origin.y + rect.size.y / 2.0)
        };
        assert_eq!(
            tab.hit(
                body(),
                centre(ListItem::Preset(BuiltinAgentPresetKey::OpenAi))
            ),
            AgentSettingsHit::SelectApiPreset(BuiltinAgentPresetKey::OpenAi)
        );
        assert_eq!(
            tab.hit(body(), centre(ListItem::Cli(AgentProvider::CodexCli))),
            AgentSettingsHit::SelectCliEntry(AgentProvider::CodexCli)
        );
        assert_eq!(
            tab.hit(body(), centre(ListItem::AddAcp)),
            AgentSettingsHit::AddAcpAgent
        );
    }

    #[test]
    fn the_default_selection_shows_the_first_cli_agent() {
        let settings = AgentSettings::default();
        let ui = EditorUiState::default();
        let tab = tab(&settings, &ui, true);
        let (_, detail) = tab.columns(body());
        let layout = cli::cli_layout(detail, &settings, &ui, &[], AgentProvider::ClaudeCode);
        let point = Point2D::new(layout.connect.origin.x + 5.0, layout.connect.origin.y + 5.0);
        assert_eq!(
            tab.hit(body(), point),
            AgentSettingsHit::Connect(AgentProvider::ClaudeCode)
        );
    }

    #[test]
    fn the_body_is_as_tall_as_its_taller_column() {
        let settings = AgentSettings::default();
        let ui = EditorUiState::default();
        let tab = tab(&settings, &ui, true);
        assert!(tab.body_height(body()) >= list::list_height(&settings, true));
    }

    #[test]
    fn a_focused_api_key_maps_to_its_input() {
        let mut settings = AgentSettings::default();
        settings.add_builtin_agent_with_defaults("Mine", "sk-1", "m");
        settings.focus = Some(SettingsFocus::BuiltinAgent {
            index: 0,
            field: de_editor_core::agent_settings::BuiltinAgentField::ApiKey,
        });
        let ui = EditorUiState::default();
        let tab = tab(&settings, &ui, false);
        let (_, detail) = tab.columns(body());
        let target = api::api_target(
            &settings,
            &AgentsEntry::Builtin(settings.builtin_agents[0].id.clone()),
        )
        .unwrap();
        let layout = api::api_layout(detail, &settings, &ui, target);
        assert_eq!(tab.focused_input_rect(body()), Some(layout.api_key));
    }
}
