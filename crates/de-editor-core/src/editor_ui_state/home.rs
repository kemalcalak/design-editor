//! Home state: the six design kinds, their per-kind drafts and canvas
//! sizes, and the prompt contracts that steer the orchestrator's
//! design-type detection.
//!
//! Home is an entry surface over the same `EditorState` as the canvas. It
//! owns only transient chrome state: which kind is active, one draft per
//! kind (text + size), the open popovers, and the small prompt wrapper
//! that turns a kind selection into the existing chat-design request
//! shape.

use crate::agent_settings::McpCli;
use jian_core::text_input::TextInputState;

/// The persisted first-launch entry preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntrySurface {
    #[default]
    Home,
    Canvas,
}

impl EntrySurface {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Canvas => "canvas",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        match value {
            "canvas" => Self::Canvas,
            _ => Self::Home,
        }
    }
}

/// Home's design kinds. The discriminant order is the chip order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HomeFamily {
    #[default]
    LandingPage,
    MobileApp,
    WebApp,
    Slides,
    Dashboard,
    /// "Something else…": a free brief with no kind contract.
    Other,
}

impl HomeFamily {
    pub const ALL: [Self; 6] = [
        Self::LandingPage,
        Self::MobileApp,
        Self::WebApp,
        Self::Slides,
        Self::Dashboard,
        Self::Other,
    ];

    /// Stable persistence / i18n id (`home.kind.<id>`).
    pub const fn id(self) -> &'static str {
        match self {
            Self::LandingPage => "landing",
            Self::MobileApp => "mobile",
            Self::WebApp => "webapp",
            Self::Slides => "slides",
            Self::Dashboard => "dashboard",
            Self::Other => "other",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|f| f.id() == value)
    }

    /// The sizes the size chip offers for this kind, in menu order.
    pub const fn sizes(self) -> &'static [HomeSize] {
        match self {
            Self::Slides => &[HomeSize::Slide169, HomeSize::Slide43],
            _ => &[
                HomeSize::Desktop1440,
                HomeSize::Tablet768,
                HomeSize::Mobile390,
            ],
        }
    }

    /// The size a fresh draft of this kind starts with.
    pub const fn default_size(self) -> HomeSize {
        match self {
            Self::MobileApp => HomeSize::Mobile390,
            Self::Slides => HomeSize::Slide169,
            _ => HomeSize::Desktop1440,
        }
    }

    /// Wrap the kind's draft with the contract the orchestrator's
    /// `detect_design_type` and the desktop launch path understand.
    ///
    /// Every contract except `Other` names a component disqualifier
    /// ("page", "screen", "app", "slides", "dashboard"), so a brief that
    /// mentions a card or a chart is not shrunk to a single component.
    /// The order of precedence in the detector is slides → mobile →
    /// dashboard/workspace → landing, which is why the web app contract
    /// says "workspace".
    pub fn generation_prompt(self, draft: &TaskDraft) -> Option<String> {
        let text = draft.text.trim();
        if text.is_empty() {
            return None;
        }
        let size = draft.size.prompt_text();
        let prompt = match self {
            Self::LandingPage => format!(
                "Design a complete, vertically scrolling landing page ({size}). Organise it \
in sections from the hero to the footer, with one consistent visual system and editable \
layers.\n\nBrief: {text}"
            ),
            Self::MobileApp => format!(
                "Design a set of editable, high-fidelity mobile app screens ({size}). Deliver \
complete screens with shared components, variables and a clean layer structure, and link \
the screens with onTap navigation.\n\nBrief: {text}"
            ),
            Self::WebApp => format!(
                "Design an editable, high-fidelity web app workspace ({size}). Deliver the main \
screens with shared components, variables and a clean layer structure.\n\nBrief: {text}"
            ),
            Self::Slides => format!(
                "Design a {}-slide presentation deck ({size}). Keep the cover, content and \
closing slides in one style, with text and graphics as editable layers.\n\nBrief: {text}",
                slide_count(text)
            ),
            Self::Dashboard => format!(
                "Design an editable, high-fidelity dashboard ({size}) with clear metrics, \
charts and tables, shared components and a clean layer structure.\n\nBrief: {text}"
            ),
            Self::Other => {
                format!("{text}\n\nCanvas: {size}. Keep every part as editable layers.")
            }
        };
        Some(prompt)
    }
}

/// Slide count a deck wrapper names: the first small number in the
/// draft (e.g. "5 slides"), else the 5-slide default.
fn slide_count(text: &str) -> u32 {
    let mut digits: Option<String> = None;
    // The trailing space flushes a number that ends the draft.
    for character in text.chars().chain([' ']) {
        if character.is_ascii_digit() {
            digits.get_or_insert_with(String::new).push(character);
            continue;
        }
        if let Some(run) = digits.take() {
            if let Ok(count) = run.parse::<u32>() {
                if (1..=30).contains(&count) {
                    return count;
                }
            }
        }
    }
    5
}

/// The canvas size the size chip writes into the prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HomeSize {
    #[default]
    Desktop1440,
    Tablet768,
    Mobile390,
    Slide169,
    Slide43,
}

impl HomeSize {
    /// Stable persistence / i18n id (`home.size.<id>`).
    pub const fn id(self) -> &'static str {
        match self {
            Self::Desktop1440 => "desktop",
            Self::Tablet768 => "tablet",
            Self::Mobile390 => "mobile",
            Self::Slide169 => "wide",
            Self::Slide43 => "classic",
        }
    }

    /// The number the chip shows after the size name ("Desktop 1440").
    pub const fn chip_suffix(self) -> &'static str {
        match self {
            Self::Desktop1440 => "1440",
            Self::Tablet768 => "768",
            Self::Mobile390 => "390",
            Self::Slide169 => "16:9",
            Self::Slide43 => "4:3",
        }
    }

    /// The menu row's detail column.
    pub const fn detail(self) -> &'static str {
        match self {
            Self::Desktop1440 => "1440 px",
            Self::Tablet768 => "768 px",
            Self::Mobile390 => "390 × 844",
            Self::Slide169 => "1920 × 1080",
            Self::Slide43 => "1440 × 1080",
        }
    }

    pub const fn is_slide(self) -> bool {
        matches!(self, Self::Slide169 | Self::Slide43)
    }

    /// The size phrase the prompt contracts embed. The phone size says
    /// neither "mobile" nor a `W×H` pair: the detector reads both as the
    /// mobile-app preset, which would turn a 390 px landing page into app
    /// screens. The mobile app contract names "mobile" itself.
    pub const fn prompt_text(self) -> &'static str {
        match self {
            Self::Desktop1440 => "desktop, 1440 px wide",
            Self::Tablet768 => "tablet, 768 px wide",
            Self::Mobile390 => "390 px wide",
            Self::Slide169 => "16:9, 1920×1080",
            Self::Slide43 => "4:3, 1440×1080",
        }
    }
}

/// How the Recent section lists files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RecentView {
    #[default]
    Grid,
    List,
}

impl RecentView {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        match value {
            "list" => Self::List,
            _ => Self::Grid,
        }
    }
}

/// A template entry the sidebar opens on a fresh document. Home asks for
/// a new file and parks this here; the host's New handler applies it to
/// the replacement state (see `EditorUiState::apply_home_template`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomeTemplate {
    DesignSystems,
    SceneTemplates,
}

/// One kind's kept draft: the text plus the chosen canvas size.
/// Attachments deliberately live in the chat composer's
/// `pending_attachments` — that list is the single source of truth.
#[derive(Debug, Clone, Default)]
pub struct TaskDraft {
    pub text: String,
    pub size: HomeSize,
}

impl TaskDraft {
    fn for_family(family: HomeFamily) -> Self {
        Self {
            text: String::new(),
            size: family.default_size(),
        }
    }
}

/// Most files Home's Recent section lists (two grid rows of four).
pub const HOME_RECENT_MAX: usize = 8;

/// A copy button on Home, for the short "Copied" acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomeCopyTarget {
    Endpoint,
    Snippet,
}

/// How long a copy button reads "Copied" after a press.
pub const HOME_COPIED_MS: u64 = 1_500;

/// Interactive target ids used by Home hover, pressed feedback and the
/// host's press dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomeHit {
    /// The composer's input box (caret press).
    Sheet,
    /// One of the kind chips above the composer.
    Kind(HomeFamily),
    /// The composer's size chip (opens the size menu).
    SizeChip,
    /// A row of the open size menu.
    SizeOption(HomeSize),
    /// A press inside the size menu that hits no row.
    SizeMenuInside,
    /// The composer's `+` (the chat attachment picker).
    Attach,
    /// The model button on the composer toolbar.
    ModelChip,
    /// "Add a model provider →", shown instead of the model button when
    /// no chat agent can answer.
    AddProvider,
    /// The round send button.
    Send,
    ImportFigma,
    ImportWeb,
    ImportScreenshot,
    ImportPenDev,
    /// A Recent card or row.
    Recent(usize),
    /// One half of the grid / list toggle.
    RecentView(RecentView),
    NavRecents,
    NavOpenFile,
    NavDesignSystems,
    NavSceneTemplates,
    NavSettings,
    /// The sidebar's MCP status card (opens the connect popover).
    McpCard,
    /// The top bar's language button (opens the shared locale picker).
    Locale,
    /// The top bar's "Open file".
    OpenFile,
    /// The top bar's accent "New file".
    NewFile,
    McpCopyEndpoint,
    /// One of the connect popover's agent tabs, or a row of its ▾ menu.
    McpAgent(McpCli),
    /// The ▾ at the end of the agent tabs.
    McpAgentMore,
    /// "Set up" — writes the selected agent's config.
    McpSetUp,
    /// "Remove" — the set-up agent's undo.
    McpRemove,
    McpCopySnippet,
    /// "MCP settings →".
    McpSettings,
    /// "Start server" while MCP is stopped.
    McpStart,
    /// A press inside the connect popover that hits no control.
    McpInside,
}

/// Transient state for the Home surface.
#[derive(Debug, Clone)]
pub struct HomeState {
    pub visible: bool,
    /// Home was opened over a document (File ▸ Home), so Escape returns
    /// to it. The launch-time Home has nothing behind it to go back to.
    pub dismissible: bool,
    /// The always-selected kind the composer edits.
    pub task: HomeFamily,
    /// One draft per kind; `draft`/`input` mirror the active one.
    pub drafts: [TaskDraft; 6],
    pub draft: String,
    pub hover: Option<HomeHit>,
    pub pressed: Option<HomeHit>,
    /// Scroll offset for a page that is taller than the viewport. The top
    /// bar and sidebar stay pinned while the column scrolls under them.
    pub scroll_y: f32,
    /// The same caret/selection machinery used by the chat composer. `draft`
    /// remains the public Home contract; this field keeps native text input,
    /// IME, clipboard, and caret edits lossless.
    pub input: TextInputState,
    pub size_menu_open: bool,
    /// The sidebar MCP card's "Connect an agent" popover.
    pub mcp_popover_open: bool,
    /// The agent tab the popover shows.
    pub mcp_agent: McpCli,
    /// The ▾ list of the agents without a tab of their own.
    pub mcp_agent_menu_open: bool,
    /// The copy button that last fired and when, for "Copied".
    pub copied: Option<(HomeCopyTarget, u64)>,
    pub recent_view: RecentView,
    /// The last MCP client that sent `initialize`, as it named itself.
    pub mcp_client: Option<String>,
    /// A sidebar template waiting for the fresh document it asked for.
    pub pending_template: Option<HomeTemplate>,
    /// The stamp of each Recent file's installed preview, by path. The host
    /// fills it as previews land; a Recent card without one shows a glyph.
    pub preview_stamps: std::collections::BTreeMap<String, u64>,
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            visible: false,
            dismissible: false,
            task: HomeFamily::LandingPage,
            drafts: HomeFamily::ALL.map(TaskDraft::for_family),
            draft: String::new(),
            hover: None,
            pressed: None,
            scroll_y: 0.0,
            input: TextInputState::default(),
            size_menu_open: false,
            mcp_popover_open: false,
            mcp_agent: McpCli::ClaudeCode,
            mcp_agent_menu_open: false,
            copied: None,
            recent_view: RecentView::Grid,
            mcp_client: None,
            pending_template: None,
            preview_stamps: std::collections::BTreeMap::new(),
        }
    }
}

impl HomeState {
    /// Show Home. `over_document` marks it as opened from the canvas, so
    /// Escape may return there.
    pub fn show(&mut self, over_document: bool) {
        self.visible = true;
        self.dismissible = over_document;
        self.scroll_y = 0.0;
    }

    /// Leave the surface; hover, pressed and every popover drop with it.
    pub fn hide(&mut self) {
        self.visible = false;
        self.dismissible = false;
        self.hover = None;
        self.pressed = None;
        self.close_popovers();
    }

    /// Step aside for a document edit that arrived from outside the UI
    /// (a live MCP client), so the user sees it land on the canvas. Only
    /// this session's surface changes; the persisted `entry_surface`
    /// preference is left alone. Returns whether Home was showing.
    pub fn reveal_for_external_edit(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        self.hide();
        true
    }

    /// Close every Home popover. Returns whether one was open.
    pub fn close_popovers(&mut self) -> bool {
        let was_open = self.size_menu_open || self.mcp_popover_open || self.mcp_agent_menu_open;
        self.size_menu_open = false;
        self.mcp_popover_open = false;
        self.mcp_agent_menu_open = false;
        was_open
    }

    /// Escape's Home rung: the innermost popover first, then Home itself
    /// when it was opened over a document. Returns whether it acted.
    pub fn escape(&mut self) -> bool {
        if self.mcp_agent_menu_open {
            self.mcp_agent_menu_open = false;
            return true;
        }
        if self.close_popovers() {
            return true;
        }
        if self.visible && self.dismissible {
            self.hide();
            return true;
        }
        false
    }

    /// Whether `target` should read "Copied" at `now_ms`.
    pub fn copied_recently(&self, target: HomeCopyTarget, now_ms: u64) -> bool {
        self.copied
            .is_some_and(|(hit, at)| hit == target && now_ms.saturating_sub(at) < HOME_COPIED_MS)
    }

    /// The next frame instant the "Copied" label needs to revert.
    pub fn copied_deadline_ms(&self, now_ms: u64) -> Option<u64> {
        let (_, at) = self.copied?;
        let until = at.saturating_add(HOME_COPIED_MS);
        (self.visible && now_ms < until).then_some(until)
    }

    pub fn task_draft(&self) -> &TaskDraft {
        self.draft_for(self.task)
    }

    /// The kept draft of any kind, active or not.
    pub fn draft_for(&self, family: HomeFamily) -> &TaskDraft {
        &self.drafts[family_index(family)]
    }

    fn task_draft_mut(&mut self) -> &mut TaskDraft {
        &mut self.drafts[family_index(self.task)]
    }

    /// Switch the active kind. The live input text is saved into the old
    /// kind's draft and the new one is loaded into `input`. Returns
    /// whether anything changed.
    pub fn set_task(&mut self, family: HomeFamily) -> bool {
        self.size_menu_open = false;
        if self.task == family {
            return false;
        }
        self.task_draft_mut().text = self.input.text().to_string();
        self.task = family;
        let text = self.task_draft().text.clone();
        self.input.set_text(text.clone());
        self.draft = text;
        true
    }

    /// Pick the active kind's canvas size (ignored when the kind does not
    /// offer it). Closes the size menu either way.
    pub fn set_size(&mut self, size: HomeSize) {
        self.size_menu_open = false;
        if self.task.sizes().contains(&size) {
            self.task_draft_mut().size = size;
        }
    }

    pub fn set_draft(&mut self, draft: impl Into<String>) {
        self.draft = draft.into();
        self.input.set_text(self.draft.clone());
        self.task_draft_mut().text = self.draft.clone();
    }

    fn sync_input_into_draft(&mut self) {
        self.draft = self.input.text().to_string();
        self.task_draft_mut().text = self.draft.clone();
    }

    /// Fold a committed IME composition (or any external input-state
    /// write) back into the public draft mirror + the active kind's
    /// kept draft.
    pub fn sync_committed_input(&mut self) {
        self.sync_input_into_draft();
    }

    pub fn insert_text(&mut self, text: &str, now_ms: u64) -> bool {
        if text.is_empty() {
            return false;
        }
        self.input.insert_str(text, now_ms);
        self.sync_input_into_draft();
        true
    }

    pub fn backspace(&mut self, now_ms: u64) -> bool {
        let before = (self.input.text().to_string(), self.input.selection());
        self.input.backspace(now_ms);
        let changed = before != (self.input.text().to_string(), self.input.selection());
        if changed {
            self.sync_input_into_draft();
        }
        changed
    }

    pub fn delete_forward(&mut self, now_ms: u64) -> bool {
        let before = (self.input.text().to_string(), self.input.selection());
        self.input.delete_forward(now_ms);
        let changed = before != (self.input.text().to_string(), self.input.selection());
        if changed {
            self.sync_input_into_draft();
        }
        changed
    }

    pub fn move_caret(&mut self, forward: bool, extend: bool, now_ms: u64) {
        if forward {
            self.input.move_right(extend, now_ms);
        } else {
            self.input.move_left(extend, now_ms);
        }
    }

    pub fn select_all(&mut self, now_ms: u64) {
        self.input.select_all();
        self.input.touch(now_ms);
    }

    pub fn set_caret(&mut self, offset: usize, now_ms: u64) {
        self.input
            .set_caret(offset.min(self.input.text().len()), now_ms);
    }

    pub fn generation_prompt(&self) -> Option<String> {
        self.task.generation_prompt(self.task_draft())
    }
}

impl super::EditorUiState {
    /// Open the panel a sidebar template entry asked for, on the fresh
    /// document the host just installed.
    pub fn apply_home_template(&mut self, template: HomeTemplate, now_ms: u64) {
        match template {
            HomeTemplate::DesignSystems => {
                // The two share the left rail slot.
                self.variables_panel_open = false;
                self.design_md_panel.open = true;
            }
            HomeTemplate::SceneTemplates => self.open_scene_template_center(now_ms),
        }
    }
}

fn family_index(family: HomeFamily) -> usize {
    HomeFamily::ALL
        .iter()
        .position(|candidate| *candidate == family)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
