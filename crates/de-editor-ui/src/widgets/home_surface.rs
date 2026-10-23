//! The Home surface (issue #22, "Home v2 — prompt first"): sidebar,
//! top bar, kind chips, the prompt composer, import cards and the
//! Recent grid / list.
//!
//! Geometry lives in `home_surface_layout.rs` and paint in the
//! `home_surface_paint*` family; this spine gathers the state they read
//! and owns hit-testing. The surface is document-agnostic: it reads the
//! same `EditorState` as the canvas and never creates a second model.

use crate::widgets::{LayoutBox, LayoutCx, PaintCx, Widget, WidgetId};
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::McpCli;
use de_editor_core::{
    EditorState, EditorUiState, HomeFamily, HomeHit, HomeState, Locale, RecentView,
};

#[path = "home_surface_palette.rs"]
mod palette;
pub use palette::{chrome_theme, HomePalette};
pub(crate) use palette::{fade, MONO, SANS};

#[path = "home_surface_icons.rs"]
mod icons;

#[path = "home_surface_copy.rs"]
mod copy;
/// Backend-free text width, shared with the settings modal's hit geometry.
pub(crate) use copy::estimate_width;
pub use copy::size_chip_label;

#[path = "home_surface_model.rs"]
mod model;
pub use model::{
    home_model_picker_rects, model_chip_label, HOME_MODEL_PICKER_GAP, HOME_MODEL_PICKER_W,
};

#[path = "home_surface_layout.rs"]
pub(crate) mod layout;
pub use layout::{
    home_layout, home_max_scroll, HomeLayout, HomeLayoutInput, HOME_TOPBAR_H, RAIL_BREAK_W,
    RECENT_MAX, WINDOW_STRIP_H,
};

#[path = "home_surface_mcp_popover.rs"]
mod mcp_popover;
pub use mcp_popover::{mcp_popover_layout, McpPopoverLayout};

#[path = "home_surface_composer.rs"]
mod composer;
#[path = "home_surface_imports.rs"]
mod imports;
#[path = "home_surface_paint.rs"]
mod paint;
#[path = "home_surface_recent.rs"]
mod recent;
#[path = "home_surface_sidebar.rs"]
mod sidebar;
#[path = "home_surface_top_bar.rs"]
mod top_bar;

/// One Recent entry as Home lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeRecent {
    /// The full path, for opening and for the thumbnail lookup.
    pub path: String,
    /// The file name without its folder or `.de` extension.
    pub name: String,
    /// The folder, `~`-abbreviated when it sits under the home dir.
    pub folder: String,
    /// The folder's own name, which the list view leads the file name
    /// with (`design / landing`, as Pencil does); empty at a root.
    pub parent: String,
    /// Unix seconds when last touched.
    pub modified_at: u64,
    /// The installed preview's stamp, when there is one.
    pub preview_stamp: Option<u64>,
}

/// What the MCP card and its popover show.
#[derive(Debug, Clone, PartialEq)]
pub struct HomeMcpView {
    pub running: bool,
    pub port: u16,
    /// The URL agents connect to (`AgentSettings::mcp_endpoint_url`).
    pub endpoint: String,
    /// The popover's selected agent and whether its config is written.
    pub agent: McpCli,
    pub agent_enabled: bool,
    /// The last client that sent `initialize`, as it named itself.
    pub client: Option<String>,
}

pub struct HomeSurface<'a> {
    pub id: WidgetId,
    pub palette: HomePalette,
    pub locale: Locale,
    pub state: &'a HomeState,
    pub ui: &'a EditorUiState,
    /// The host's frame clock (caret blink, "Copied" timeouts).
    pub now_ms: u64,
    /// Unix seconds for the Recent ages; `0` paints no age.
    pub wall_secs: u64,
    /// "<model> · <provider>", empty when no agent can answer.
    pub chip_label: String,
    /// Whether any chat agent can answer — swaps the model button for the
    /// provider link and disables send.
    pub usable_agent: bool,
    /// Names of the chat composer's staged attachments (the shared
    /// `chat.pending_attachments` list is the single source of truth).
    pub attachment_names: Vec<String>,
    pub recent: Vec<HomeRecent>,
    pub mcp: HomeMcpView,
}

impl<'a> HomeSurface<'a> {
    pub fn for_editor(state: &'a EditorState) -> Option<Self> {
        Self::for_editor_at(state, 0)
    }

    pub fn for_editor_at(state: &'a EditorState, now_ms: u64) -> Option<Self> {
        let ui = &state.editor_ui;
        let home = &ui.home;
        home.visible.then(|| Self {
            id: WidgetId::new(7600),
            palette: HomePalette::for_mode(ui.effective_theme_mode()),
            locale: ui.effective_locale(),
            state: home,
            ui,
            now_ms,
            wall_secs: 0,
            chip_label: model::model_chip_label(state),
            usable_agent: state.has_usable_chat_agent(),
            attachment_names: state
                .chat
                .pending_attachments
                .iter()
                .map(|attachment| attachment.name.clone())
                .collect(),
            recent: ui
                .recent_files
                .iter()
                .take(RECENT_MAX)
                .map(|file| {
                    let stamp = home.preview_stamps.get(&file.path).copied();
                    recent_entry(&file.path, file.modified_at, stamp)
                })
                .collect(),
            mcp: HomeMcpView {
                running: ui.agent_settings.mcp_server.running,
                port: ui.agent_settings.mcp_server.port,
                endpoint: ui.agent_settings.mcp_endpoint_url(),
                agent: home.mcp_agent,
                agent_enabled: ui.agent_settings.mcp_cli_enabled[home.mcp_agent.index()],
                client: home.mcp_client.clone(),
            },
        })
    }

    /// Stamp the wall clock the Recent ages are measured against.
    pub fn with_wall_clock(mut self, wall_secs: u64) -> Self {
        self.wall_secs = wall_secs;
        self
    }

    pub fn layout_input(&self, viewport_width: f32, viewport_height: f32) -> HomeLayoutInput<'_> {
        HomeLayoutInput {
            viewport_w: viewport_width,
            viewport_h: viewport_height,
            scroll_y: 0.0,
            locale: self.locale,
            task: self.state.task,
            size: self.state.task_draft().size,
            model_label: &self.chip_label,
            usable_agent: self.usable_agent,
            recent_count: self.recent.len(),
            recent_view: self.state.recent_view,
        }
    }

    pub fn max_scroll(&self, viewport_width: f32, viewport_height: f32) -> f32 {
        home_max_scroll(self.layout_input(viewport_width, viewport_height))
    }

    pub fn layout(&self, viewport_width: f32, viewport_height: f32) -> HomeLayout {
        let max = self.max_scroll(viewport_width, viewport_height);
        home_layout(HomeLayoutInput {
            scroll_y: self.state.scroll_y.clamp(0.0, max),
            ..self.layout_input(viewport_width, viewport_height)
        })
    }

    /// The connect popover's rects (only meaningful while it is open).
    pub fn mcp_popover(&self, layout: &HomeLayout, viewport_width: f32) -> McpPopoverLayout {
        mcp_popover_layout(self, layout, viewport_width)
    }

    /// Whether a press at `point` lands on an open popover. A press
    /// outside every open popover only closes them (the host's job).
    pub fn popover_contains(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        point: Point2D,
    ) -> bool {
        let layout = self.layout(viewport_width, viewport_height);
        if self.state.mcp_popover_open {
            let popover = self.mcp_popover(&layout, viewport_width);
            if popover.contains(point, self.state.mcp_agent_menu_open) {
                return true;
            }
        }
        self.state.size_menu_open && layout.size_menu.contains(point)
    }

    pub fn hit_test(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        point: Point2D,
    ) -> Option<HomeHit> {
        let layout = self.layout(viewport_width, viewport_height);
        if self.state.mcp_popover_open {
            let popover = self.mcp_popover(&layout, viewport_width);
            if let Some(hit) = popover.hit_test(point, self.state.mcp_agent_menu_open) {
                return Some(hit);
            }
        }
        if self.state.size_menu_open && layout.size_menu.contains(point) {
            let sizes = self.state.task.sizes();
            for (index, rect) in layout.size_rows.iter().enumerate() {
                if rect.contains(point) {
                    return sizes.get(index).copied().map(HomeHit::SizeOption);
                }
            }
            return Some(HomeHit::SizeMenuInside);
        }
        if layout.new_file.contains(point) {
            return Some(HomeHit::NewFile);
        }
        if layout.open_file.contains(point) {
            return Some(HomeHit::OpenFile);
        }
        if layout.locale_button.contains(point) {
            return Some(HomeHit::Locale);
        }
        if layout.sidebar.contains(point) {
            const NAV: [HomeHit; 5] = [
                HomeHit::NavRecents,
                HomeHit::NavOpenFile,
                HomeHit::NavDesignSystems,
                HomeHit::NavSceneTemplates,
                HomeHit::NavSettings,
            ];
            for (rect, hit) in layout.nav.iter().zip(NAV) {
                if rect.contains(point) {
                    return Some(hit);
                }
            }
            return layout.mcp_card.contains(point).then_some(HomeHit::McpCard);
        }
        // The column scrolls under the top bar; nothing there is live.
        if !layout.viewport_body.contains(point) {
            return None;
        }
        for (rect, family) in layout.chips.iter().zip(HomeFamily::ALL) {
            if rect.contains(point) {
                return Some(HomeHit::Kind(family));
            }
        }
        if layout.attach.contains(point) {
            return Some(HomeHit::Attach);
        }
        if layout.size_chip.contains(point) {
            return Some(HomeHit::SizeChip);
        }
        if layout.send.contains(point) {
            return Some(HomeHit::Send);
        }
        if layout.model_chip.contains(point) {
            return Some(if self.usable_agent {
                HomeHit::ModelChip
            } else {
                HomeHit::AddProvider
            });
        }
        if layout.composer.contains(point) {
            return Some(HomeHit::Sheet);
        }
        const IMPORTS: [HomeHit; 4] = [
            HomeHit::ImportFigma,
            HomeHit::ImportWeb,
            HomeHit::ImportScreenshot,
            HomeHit::ImportPenDev,
        ];
        for (rect, hit) in layout.imports.iter().zip(IMPORTS) {
            if rect.contains(point) {
                return Some(hit);
            }
        }
        if layout.view_grid.contains(point) {
            return Some(HomeHit::RecentView(RecentView::Grid));
        }
        if layout.view_list.contains(point) {
            return Some(HomeHit::RecentView(RecentView::List));
        }
        layout
            .recent_items
            .iter()
            .position(|rect| rect.contains(point))
            .map(HomeHit::Recent)
    }

    /// A press in the window's title strip that hits no Home control
    /// starts a window drag (the window is borderless).
    pub fn window_drag_at(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        point: Point2D,
    ) -> bool {
        point.y < WINDOW_STRIP_H
            && !self.popover_contains(viewport_width, viewport_height, point)
            && self
                .hit_test(viewport_width, viewport_height, point)
                .is_none()
    }

    /// The IME caret rect inside the composer's input box.
    pub fn focused_input_caret_rect(&self, viewport_width: f32, viewport_height: f32) -> Rect {
        let layout = self.layout(viewport_width, viewport_height);
        composer::caret_rect(self, &layout)
    }
}

/// Split a recent path into the name and `~`-abbreviated folder Home
/// shows.
fn recent_entry(path: &str, modified_at: u64, preview_stamp: Option<u64>) -> HomeRecent {
    let split = path.rfind(['/', '\\']);
    let (folder, file) = match split {
        Some(index) => (&path[..index], &path[index + 1..]),
        None => ("", path),
    };
    // pen.dev imports stay listed under their `.pen` source.
    let name = de_util::document_file::strip_known_suffix(file);
    let folder = home_dir_prefix(folder)
        .map(|rest| format!("~{rest}"))
        .unwrap_or_else(|| folder.to_string());
    let parent = folder.rsplit(['/', '\\']).next().unwrap_or("").to_string();
    HomeRecent {
        path: path.to_string(),
        name: name.to_string(),
        folder,
        parent,
        modified_at,
        preview_stamp,
    }
}

/// The part of `folder` after the user's home directory, when it sits
/// under one (`/Users/<name>`, `/home/<name>`, `C:\Users\<name>`).
fn home_dir_prefix(folder: &str) -> Option<&str> {
    for root in ["/Users/", "/home/", "C:\\Users\\", "C:/Users/"] {
        if let Some(rest) = folder.strip_prefix(root) {
            let end = rest.find(['/', '\\']).unwrap_or(rest.len());
            return Some(&rest[end..]);
        }
    }
    None
}

impl Widget for HomeSurface<'_> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, cx: &LayoutCx) -> LayoutBox {
        LayoutBox {
            rect: Rect::xywh(0.0, 0.0, cx.available_width, 900.0),
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        paint::paint_home(self, cx, rect);
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Main);
        node.set_label(copy::tr(self.locale, "home.title"));
        node
    }
}

#[cfg(test)]
#[path = "home_surface_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "home_surface_layout_tests.rs"]
mod layout_tests;
