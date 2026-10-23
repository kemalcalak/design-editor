use crate::theme::Theme;
use crate::widgets::agent_settings_fonts::{self, FontsHit};
use crate::widgets::agent_settings_i18n::t as t_settings;
use crate::widgets::agent_settings_images::{self, ImagesHit};
use crate::widgets::agent_settings_mcp::{self, McpHit};
use crate::widgets::agent_settings_panel_geometry::{
    close_rect, content_rect, hero_body_rect, hero_body_rect_for_ui, tab_i18n_label,
};
use crate::widgets::agent_settings_system::{self, SystemHit};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::{PaintCx, Widget, WidgetId};
use crate::{Point2D, Rect, TextLayout};
use de_editor_core::agent_settings::{
    AcpAgentField, AgentProvider, AgentSettings, AgentSettingsTab, BuiltinAgentField,
    ImageGenField, ImageSearchField, McpCli,
};
use de_editor_core::editor_ui_state::EditorUiState;
use de_editor_core::BuiltinAgentPresetKey;
use de_editor_core::EditorState;
use de_editor_core::{AgentSettingsButton, ButtonPressTarget};

/// Modal size ceiling. The dialog is a centred workspace rather than a
/// fixed small window: [`AgentSettingsPanel::rect`] shrinks it to fit the
/// viewport, so these are maxima, not fixed dimensions.
///
/// They came down with the row scale. A 1100×850 shell around 54 px rows
/// is mostly empty column — the settings read as a poster instead of a
/// list, which is the "too big" the shipped build showed.
pub const PANEL_WIDTH: f32 = 960.0;
pub const PANEL_HEIGHT: f32 = 760.0;
/// Floor for the shrink-to-fit clamp — below this the tab strip and the
/// provider rows stop being readable, so the modal overflows a tiny
/// viewport instead of collapsing.
const PANEL_MIN_WIDTH: f32 = 620.0;
const PANEL_MIN_HEIGHT: f32 = 420.0;
/// Breathing room kept between the modal and the viewport edges.
const VIEWPORT_MARGIN_X: f32 = 32.0;
const VIEWPORT_MARGIN_BOTTOM: f32 = 48.0;

// The modal's spacing scale lives in `agent_settings_metrics`; the names
// below are that scale under the spellings this file's callers already
// use, plus the few control sizes that belong to a specific control.
use crate::widgets::agent_settings_metrics as metrics;

pub(super) const PAD: f32 = metrics::CONTENT_PAD_X;
pub(super) const CONTENT_BOTTOM_PAD: f32 = metrics::CONTENT_PAD_BOTTOM;
/// Desktop: the left navigation rail's width, and the inset from the
/// panel top to the scrollable body (the tab's own title sits in the body).
pub(super) const NAV_W: f32 = 200.0;
pub(super) const NAV_TITLE_H: f32 = 56.0;
pub(super) const NAV_ITEM_H: f32 = 34.0;
pub(super) const NAV_ITEM_GAP: f32 = 2.0;
pub(super) const HEADER_HEIGHT: f32 = 22.0;
/// Total vertical inset between the panel rect and its scrollable
/// content viewport. Hosts subtract this from the panel height to derive
/// the scroll viewport (`content_rect` height) instead of hardcoding it.
pub const CONTENT_VERTICAL_INSET: f32 = HEADER_HEIGHT + CONTENT_BOTTOM_PAD;
pub(super) const CONTENT_TAIL_PAD: f32 = metrics::CONTENT_TAIL_PAD;
/// Agents-tab intro block (title + provider roll) above the first
/// section — the vertical offset from the content viewport's top to the
/// first section header. Public so host tests can anchor to it; it must
/// stay equal to `agent_settings_rows::tab_intro_height(true)`, which a
/// unit test asserts.
pub const AGENTS_HERO_HEIGHT: f32 = crate::widgets::agent_settings_rows::tab_intro_height(true);

/// Scrollable body viewport inside `panel` — everything below the top
/// tab strip, inset by the modal's content padding. Exported so hosts and
/// their tests read the body geometry from its one definition instead of
/// re-deriving the insets.
pub fn content_viewport(panel: Rect) -> Rect {
    content_rect(panel)
}

/// Body viewport for the tabs the panel paints a hero on behalf of
/// (Images / Fonts): [`content_viewport`] pushed down past that
/// hero. Those tabs' own geometry is written against the top of their
/// body, so this is the rect they — and anything testing them — must use.
pub fn secondary_tab_body(panel: Rect) -> Rect {
    hero_body_rect(content_rect(panel))
}

/// MCP tab: the server Start/Stop button.
pub fn mcp_server_button(panel: Rect) -> Rect {
    agent_settings_mcp::server_button_rect(content_rect(panel))
}

/// MCP tab: the endpoint's Copy chip in the server card. It sits right
/// after the endpoint text, so it needs the settings that pick the URL.
pub fn mcp_copy_config_button(panel: Rect, settings: &AgentSettings) -> Rect {
    agent_settings_mcp::client_config_copy_button_rect(content_rect(panel), settings)
}

/// System tab: the switch on the Auto-update row.
pub fn system_auto_update_switch(panel: Rect) -> Rect {
    agent_settings_system::auto_update_switch_rect(content_rect(panel))
}

/// Close button rect in the header band, exported alongside
/// [`content_viewport`] so hosts and their tests target it by geometry
/// rather than by a copied offset.
pub fn close_button_rect(panel: Rect) -> Rect {
    close_rect(panel)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSettingsPanelMode {
    Full,
    WebBuiltinOnly,
    McpOnly,
}

impl AgentSettingsPanelMode {
    fn visible_tabs(self) -> &'static [AgentSettingsTab] {
        match self {
            AgentSettingsPanelMode::Full => &AgentSettingsTab::ALL,
            AgentSettingsPanelMode::WebBuiltinOnly => &[
                AgentSettingsTab::Agents,
                AgentSettingsTab::Images,
                AgentSettingsTab::Fonts,
                AgentSettingsTab::System,
            ],
            AgentSettingsPanelMode::McpOnly => &[AgentSettingsTab::Mcp],
        }
    }

    fn active_tab(self, settings: &AgentSettings) -> AgentSettingsTab {
        if self.visible_tabs().contains(&settings.tab) {
            settings.tab
        } else {
            self.visible_tabs()[0]
        }
    }

    /// Whether the Agents tab lists the agents on this machine — the CLI
    /// agents, the ACP agents and "Add ACP agent" — and their detail panes.
    ///
    /// Paint, hit-test, hover, and the content-height walk all read this
    /// one predicate so a hidden block leaves no live hit rect behind it.
    fn shows_external_agents(self) -> bool {
        self == AgentSettingsPanelMode::Full
    }
}

fn mode_for_ui(ui: &EditorUiState, base: AgentSettingsPanelMode) -> AgentSettingsPanelMode {
    if ui.embed == de_editor_core::EmbedHost::VsCode {
        AgentSettingsPanelMode::McpOnly
    } else {
        base
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSettingsHit {
    Close,
    SelectTab(AgentSettingsTab),
    Connect(AgentProvider),
    /// "Add custom endpoint": a draft on the Custom preset.
    AddProvider,
    /// Agents tab list rows (#26).
    SelectBuiltinEntry(usize),
    /// A featured preset not set up yet: a draft on that preset.
    SelectApiPreset(BuiltinAgentPresetKey),
    SelectCliEntry(AgentProvider),
    SelectAcpEntry(usize),
    ToggleApiKeyReveal,
    /// Force a catalog refresh for a saved provider or the draft.
    FetchBuiltinModels(Option<usize>),
    /// Probe a CLI agent again without disconnecting it first.
    RecheckProvider(AgentProvider),
    /// Tick or untick a CLI agent's model — positional over the agent's
    /// discovered models for the same frame.
    ToggleCliModel {
        provider: AgentProvider,
        row: usize,
    },
    /// Fill the ACP draft from a preset (`None` = Custom).
    SelectAcpPreset(Option<usize>),
    SetAcpConnection {
        index: Option<usize>,
        kind: de_editor_core::agent_settings::AcpConnectionType,
    },
    FocusBuiltinAgent {
        index: usize,
        field: BuiltinAgentField,
    },
    FocusBuiltinAgentDraft(BuiltinAgentField),
    ToggleBuiltinAgentKind(usize),
    ToggleBuiltinAgentDraftKind,
    ToggleBuiltinAgentPresetMenu(Option<usize>),
    SelectBuiltinAgentPreset {
        index: Option<usize>,
        preset: BuiltinAgentPresetKey,
    },
    /// Model checklist row pressed — positional index into the pane's
    /// rows (saved models, then the catalog's others), resolved to the
    /// model id by the press arm so this enum stays `Copy`.
    ToggleBuiltinModel {
        index: Option<usize>,
        row: usize,
    },
    SaveBuiltinAgentDraft,
    /// Commit the focused field of a saved provider and refresh its catalog.
    SaveBuiltinAgentEditing(usize),
    CancelBuiltinAgentDraft,
    ToggleBuiltinAgentEnabled(usize),
    RemoveBuiltinAgent(usize),
    AddAcpAgent,
    FocusAcpAgent {
        index: usize,
        field: AcpAgentField,
    },
    FocusAcpAgentDraft(AcpAgentField),
    SaveAcpAgentDraft,
    CancelAcpAgentDraft,
    RemoveAcpAgent(usize),
    ToggleAcpConnected(usize),
    ToggleMcpServer,
    ToggleMcpCli(McpCli),
    CopyMcpClientConfig,
    /// Expand / collapse an MCP agent row's manual snippet.
    ExpandMcpCli(McpCli),
    CopyMcpSnippet(McpCli),
    ToggleMcpShowAll,
    ToggleImagesAdvanced,
    FocusSearchField(ImageSearchField),
    OpenImageRegisterLink,
    TestImageSearch,
    AddGenConfig,
    ToggleGenConfigEditor(usize),
    SetActiveGenConfig(usize),
    RemoveGenConfig(usize),
    TestGenConfig(usize),
    ToggleGenProviderMenu(usize),
    SelectGenProvider {
        index: usize,
        provider: de_editor_core::agent_settings::ImageGenProvider,
    },
    FocusGenConfig {
        index: usize,
        field: ImageGenField,
    },
    Fonts(FontsHit),
    ToggleAutoUpdate,
    ToggleExperimental,
    SelectThemeMode(de_editor_core::editor_ui_state::ThemeMode),
    SelectPencilCursor(de_editor_core::PencilCursorStyle),
    FocusMcpPort,
    Outside,
    Inside,
}

pub struct AgentSettingsPanel<'a> {
    pub id: WidgetId,
    pub theme: Theme,
    pub settings: AgentSettings,
    pub now_ms: u64,
    mode: AgentSettingsPanelMode,
    ui: &'a EditorUiState,
    /// CLI agents' discovered models, for their checklists.
    discovered: &'a [de_editor_core::ModelEntry],
}

impl<'a> AgentSettingsPanel<'a> {
    pub fn for_editor(state: &'a EditorState) -> Self {
        Self::for_editor_at(state, 0)
    }

    pub fn for_editor_at(state: &'a EditorState, now_ms: u64) -> Self {
        Self {
            id: WidgetId::new(5200),
            theme: crate::widgets::home_surface::chrome_theme(
                state.editor_ui.effective_theme_mode(),
            ),
            settings: state.editor_ui.agent_settings.clone(),
            now_ms,
            mode: mode_for_ui(&state.editor_ui, AgentSettingsPanelMode::Full),
            ui: &state.editor_ui,
            discovered: &state.chat.discovered_models,
        }
    }

    pub fn for_web_editor(state: &'a EditorState) -> Self {
        Self::for_web_editor_at(state, 0)
    }

    pub fn for_web_editor_at(state: &'a EditorState, now_ms: u64) -> Self {
        Self {
            id: WidgetId::new(5200),
            theme: crate::widgets::home_surface::chrome_theme(
                state.editor_ui.effective_theme_mode(),
            ),
            settings: state.editor_ui.agent_settings.clone(),
            now_ms,
            mode: mode_for_ui(&state.editor_ui, AgentSettingsPanelMode::WebBuiltinOnly),
            ui: &state.editor_ui,
            discovered: &state.chat.discovered_models,
        }
    }

    /// Centred modal rect, clamped to the viewport. Width and height both
    /// shrink below their ceiling when the window is small, so the dialog
    /// never runs off screen or under the top bar.
    pub fn rect(&self, viewport_w: f32, viewport_h: f32) -> Rect {
        let top_limit = crate::widgets::WINDOW_STRIP_H + 8.0;
        let w = PANEL_WIDTH
            .min(viewport_w - VIEWPORT_MARGIN_X * 2.0)
            .max(PANEL_MIN_WIDTH);
        let h = PANEL_HEIGHT
            .min(viewport_h - crate::widgets::WINDOW_STRIP_H - VIEWPORT_MARGIN_BOTTOM)
            .max(PANEL_MIN_HEIGHT);
        let x = ((viewport_w - w) / 2.0).max(8.0);
        let y = ((viewport_h - h) / 2.0).max(top_limit);
        Rect {
            origin: Point2D::new(x, y),
            size: Point2D::new(w, h),
        }
    }

    /// Resolved modal geometry shared by paint, hit-testing, scrolling, and
    /// host gesture code.
    pub fn resolved_layout(&self, panel: Rect) -> AgentSettingsLayout {
        AgentSettingsLayout::resolve(panel, self.navigation_tabs().len())
    }

    pub fn resolved_content_viewport(&self, panel: Rect) -> Rect {
        self.resolved_layout(panel).content
    }

    pub fn max_scroll(&self, panel: Rect) -> f32 {
        let content = self.resolved_content_viewport(panel);
        let total = match self.mode.active_tab(&self.settings) {
            AgentSettingsTab::Mcp => crate::widgets::agent_settings_mcp::content_height_at(
                &self.settings,
                content.size.x,
            ),
            _ => self.content_total_height(),
        };
        (total - content.size.y).max(0.0)
    }

    pub fn effective_scroll(&self, panel: Rect) -> f32 {
        self.settings
            .scroll_y
            .offset
            .clamp(0.0, self.max_scroll(panel))
    }

    pub fn navigation_tabs(&self) -> &'static [AgentSettingsTab] {
        self.mode.visible_tabs()
    }

    pub fn active_tab(&self) -> AgentSettingsTab {
        self.mode.active_tab(&self.settings)
    }

    /// The Agents tab body for this frame.
    pub(crate) fn agents_tab(&self) -> crate::widgets::agent_settings_agents::AgentsTab<'_> {
        crate::widgets::agent_settings_agents::AgentsTab {
            settings: &self.settings,
            ui: self.ui,
            discovered: self.discovered,
            external: self.mode.shows_external_agents(),
        }
    }
}

mod hero;
mod hit_test;
mod layout;
mod paint;
mod tabs;

pub use layout::AgentSettingsLayout;
pub use paint::drag_for_hit;
