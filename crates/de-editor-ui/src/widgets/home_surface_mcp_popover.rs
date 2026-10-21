//! The "Connect an agent" popover the sidebar MCP card opens: endpoint,
//! agent tabs, one-click set-up, the manual snippet, connected agents.
//! Geometry, hit-test and paint share one `McpPopoverLayout`.

use super::composer::paint_popover_card;
use super::copy::{estimate_width, tr};
use super::icons::{draw_glyph, HomeGlyph};
use super::paint::{baseline, caption, draw_text, draw_text_weighted};
use super::{HomeLayout, HomeSurface, MONO, SANS};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::agent_settings::McpCli;
use de_editor_core::home_mcp_setup::{
    home_mcp_agent_name, home_mcp_config_path, home_mcp_manual_snippet, home_mcp_more_agents,
    HOME_MCP_TABS,
};
use de_editor_core::{local_mcp_url, HomeCopyTarget, HomeHit};

const WIDTH: f32 = 440.0;
const PAD: f32 = 16.0;
const GAP: f32 = 14.0;
const LABEL_H: f32 = 15.0;
const INTRO_FONT: f32 = 12.0;
const INTRO_LINE: f32 = 18.0;
const SNIPPET_FONT: f32 = 11.0;
const SNIPPET_LINE: f32 = 17.0;
const TAB_FONT: f32 = 11.5;
const MORE_ROW_H: f32 = 30.0;
const MORE_W: f32 = 200.0;

/// Every rect and wrapped line the popover paints.
#[derive(Debug, Clone, PartialEq)]
pub struct McpPopoverLayout {
    pub card: Rect,
    pub header: Rect,
    /// "Start server" (zero while MCP runs).
    pub start: Rect,
    pub intro_lines: Vec<String>,
    pub intro_y: f32,
    pub endpoint_label_y: f32,
    pub endpoint_box: Rect,
    pub copy_endpoint: Rect,
    pub agent_label_y: f32,
    pub tabs_box: Rect,
    pub tabs: Vec<(McpCli, Rect)>,
    pub more_button: Rect,
    pub auto_box: Rect,
    /// "Set up", or "Remove" once the agent's config is written.
    pub action: Rect,
    pub action_hit: HomeHit,
    pub manual_label_y: f32,
    pub snippet_box: Rect,
    pub snippet_lines: Vec<String>,
    pub copy_snippet: Rect,
    pub divider_y: f32,
    pub connected_label_y: f32,
    pub connected_row: Rect,
    pub footer: Rect,
    pub settings_link: Rect,
    /// The ▾ list of the agents without a tab.
    pub more_menu: Rect,
    pub more_rows: Vec<(McpCli, Rect)>,
}

impl McpPopoverLayout {
    pub fn contains(&self, point: Point2D, menu_open: bool) -> bool {
        self.card.contains(point) || (menu_open && self.more_menu.contains(point))
    }

    /// The control under `point`, `McpInside` for a press on the card that
    /// hits none, `None` outside it.
    pub fn hit_test(&self, point: Point2D, menu_open: bool) -> Option<HomeHit> {
        if menu_open && self.more_menu.contains(point) {
            return Some(
                self.more_rows
                    .iter()
                    .find(|(_, rect)| rect.contains(point))
                    .map_or(HomeHit::McpInside, |(cli, _)| HomeHit::McpAgent(*cli)),
            );
        }
        if !self.card.contains(point) {
            return None;
        }
        if let Some((cli, _)) = self.tabs.iter().find(|(_, rect)| rect.contains(point)) {
            return Some(HomeHit::McpAgent(*cli));
        }
        let controls = [
            (self.start, HomeHit::McpStart),
            (self.copy_endpoint, HomeHit::McpCopyEndpoint),
            (self.more_button, HomeHit::McpAgentMore),
            (self.copy_snippet, HomeHit::McpCopySnippet),
            (self.settings_link, HomeHit::McpSettings),
        ];
        if let Some((_, hit)) = controls.iter().find(|(rect, _)| rect.contains(point)) {
            return Some(*hit);
        }
        if self.action.contains(point) {
            return Some(self.action_hit);
        }
        Some(HomeHit::McpInside)
    }
}

/// Greedy word wrap against the layout-time width estimate.
fn wrap_words(text: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && estimate_width(&candidate, size) > max_w {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Hard-wrap mono lines at the column count that fits `max_w`.
fn wrap_mono(text: &str, max_w: f32, size: f32) -> Vec<String> {
    let columns = ((max_w / (size * 0.6)).floor() as usize).max(8);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        for chunk in chars.chunks(columns) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}

fn agent_title(surface: &HomeSurface<'_>) -> String {
    tr(surface.locale, "home.mcp.addTo").replace("{{name}}", home_mcp_agent_name(surface.mcp.agent))
}

fn action_label(surface: &HomeSurface<'_>) -> &'static str {
    tr(
        surface.locale,
        if surface.mcp.agent_enabled {
            "home.mcp.remove"
        } else {
            "home.mcp.setUp"
        },
    )
}

fn copy_button_w(surface: &HomeSurface<'_>) -> f32 {
    let copy = estimate_width(tr(surface.locale, "home.mcp.copy"), 11.0);
    let copied = estimate_width(tr(surface.locale, "home.mcp.copied"), 11.0);
    8.0 + 12.0 + 5.0 + copy.max(copied) + 8.0
}

pub fn mcp_popover_layout(
    surface: &HomeSurface<'_>,
    home: &HomeLayout,
    viewport_w: f32,
) -> McpPopoverLayout {
    let x = home.sidebar.size.x + 8.0;
    let width = WIDTH.min(viewport_w - x - 8.0).max(280.0);
    let inner_x = x + PAD;
    let inner_w = width - 2.0 * PAD;
    let running = surface.mcp.running;
    let intro_key = if running {
        "home.mcp.intro"
    } else {
        "home.mcp.stoppedIntro"
    };
    let intro_lines = wrap_words(tr(surface.locale, intro_key), inner_w, INTRO_FONT);
    let snippet = home_mcp_manual_snippet(surface.mcp.agent, surface.mcp.port);
    let copy_w = copy_button_w(surface);
    let snippet_lines = wrap_mono(&snippet, inner_w - 20.0 - copy_w - 8.0, SNIPPET_FONT);

    // Lay out top-down from 0, then shift so the card's bottom meets the
    // MCP card's bottom (clamped to the viewport top).
    let mut y = PAD;
    let header = Rect::xywh(inner_x, y, inner_w, 20.0);
    let start = if running {
        Rect::ZERO
    } else {
        let w = estimate_width(tr(surface.locale, "home.mcp.start"), 11.5) + 20.0;
        Rect::xywh(inner_x + inner_w - w, y - 4.0, w, 28.0)
    };
    y += 20.0 + GAP;
    let intro_y = y;
    y += intro_lines.len() as f32 * INTRO_LINE + GAP;
    let endpoint_label_y = y;
    y += LABEL_H + 6.0;
    let endpoint_box = Rect::xywh(inner_x, y, inner_w, 36.0);
    let copy_endpoint = Rect::xywh(inner_x + inner_w - 8.0 - copy_w, y + 5.0, copy_w, 26.0);
    y += 36.0 + GAP;
    let agent_label_y = y;
    y += LABEL_H + 10.0;
    let tabs_box = Rect::xywh(inner_x, y, inner_w, 32.0);
    let more_selected = !HOME_MCP_TABS.contains(&surface.mcp.agent);
    let more_w = if more_selected {
        estimate_width(home_mcp_agent_name(surface.mcp.agent), TAB_FONT) + 40.0
    } else {
        34.0
    };
    let more_button = Rect::xywh(inner_x + inner_w - 3.0 - more_w, y + 3.0, more_w, 26.0);
    // Tabs fill left to right until they would meet the ▾ slot; the ones
    // that do not fit move into its menu, so a long selected name never
    // paints over a tab.
    let mut tabs = Vec::new();
    let mut overflow = Vec::new();
    let mut tab_x = inner_x + 3.0;
    for cli in HOME_MCP_TABS {
        let w = estimate_width(home_mcp_agent_name(cli), TAB_FONT) + 20.0;
        if overflow.is_empty() && tab_x + w <= more_button.origin.x - 4.0 {
            tabs.push((cli, Rect::xywh(tab_x, y + 3.0, w, 26.0)));
            tab_x += w + 2.0;
        } else {
            overflow.push(cli);
        }
    }
    y += 32.0 + 10.0;
    let auto_box = Rect::xywh(inner_x, y, inner_w, 56.0);
    let action_w = estimate_width(action_label(surface), 12.0) + 12.0 + 13.0 + 6.0 + 12.0;
    let action = Rect::xywh(
        inner_x + inner_w - 12.0 - action_w,
        y + 13.0,
        action_w,
        30.0,
    );
    y += 56.0 + 10.0;
    let manual_label_y = y;
    y += LABEL_H + 10.0;
    let snippet_h = (snippet_lines.len() as f32 * SNIPPET_LINE + 16.0).max(36.0);
    let snippet_box = Rect::xywh(inner_x, y, inner_w, snippet_h);
    let copy_snippet = Rect::xywh(inner_x + inner_w - 8.0 - copy_w, y + 5.0, copy_w, 26.0);
    y += snippet_h + GAP;
    let divider_y = y;
    y += 1.0 + GAP;
    let connected_label_y = y;
    y += LABEL_H + 6.0;
    let connected_row = Rect::xywh(inner_x, y, inner_w, 18.0);
    y += 18.0 + GAP;
    let footer = Rect::xywh(inner_x, y, inner_w, 18.0);
    let link_w = estimate_width(tr(surface.locale, "home.mcp.settings"), 12.0) + 18.0;
    let settings_link = Rect::xywh(inner_x + inner_w - link_w, y - 4.0, link_w, 26.0);
    y += 18.0 + PAD;
    let height = y;

    let bottom = home.mcp_card.origin.y + home.mcp_card.size.y;
    let top = (bottom - height).max(8.0);
    let shift = |r: Rect| {
        if r.size.x == 0.0 {
            r
        } else {
            Rect::xywh(r.origin.x, r.origin.y + top, r.size.x, r.size.y)
        }
    };
    let tabs: Vec<(McpCli, Rect)> = tabs.into_iter().map(|(c, r)| (c, shift(r))).collect();
    let more_button = shift(more_button);
    let more: Vec<McpCli> = overflow.into_iter().chain(home_mcp_more_agents()).collect();
    let menu_h = 8.0 + more.len() as f32 * MORE_ROW_H;
    let menu_x = more_button.origin.x + more_button.size.x - MORE_W;
    // The list opens below the ▾ when it fits, else above it.
    let below = more_button.origin.y + more_button.size.y + 4.0;
    let menu_y = if below + menu_h <= top + height + 120.0 {
        below
    } else {
        more_button.origin.y - 4.0 - menu_h
    };
    let more_menu = Rect::xywh(menu_x, menu_y, MORE_W, menu_h);
    let more_rows = more
        .into_iter()
        .enumerate()
        .map(|(i, cli)| {
            (
                cli,
                Rect::xywh(
                    menu_x + 4.0,
                    menu_y + 4.0 + i as f32 * MORE_ROW_H,
                    MORE_W - 8.0,
                    MORE_ROW_H,
                ),
            )
        })
        .collect();

    McpPopoverLayout {
        card: Rect::xywh(x, top, width, height),
        header: shift(header),
        start: shift(start),
        intro_lines,
        intro_y: intro_y + top,
        endpoint_label_y: endpoint_label_y + top,
        endpoint_box: shift(endpoint_box),
        copy_endpoint: shift(copy_endpoint),
        agent_label_y: agent_label_y + top,
        tabs_box: shift(tabs_box),
        tabs,
        more_button,
        auto_box: shift(auto_box),
        action: shift(action),
        action_hit: if surface.mcp.agent_enabled {
            HomeHit::McpRemove
        } else {
            HomeHit::McpSetUp
        },
        manual_label_y: manual_label_y + top,
        snippet_box: shift(snippet_box),
        snippet_lines,
        copy_snippet: shift(copy_snippet),
        divider_y: divider_y + top,
        connected_label_y: connected_label_y + top,
        connected_row: shift(connected_row),
        footer: shift(footer),
        settings_link: shift(settings_link),
        more_menu,
        more_rows,
    }
}

pub(super) fn paint_mcp_popover(
    surface: &HomeSurface<'_>,
    cx: &mut PaintCx<'_>,
    layout: &McpPopoverLayout,
) {
    let p = surface.palette;
    let hover = surface.state.hover;
    paint_popover_card(cx, surface, layout.card);

    // Header: status dot, title, state (or Start server).
    let header = layout.header;
    let running = surface.mcp.running;
    cx.backend.fill_oval(
        Rect::xywh(header.origin.x, header.origin.y + 6.5, 7.0, 7.0),
        if running { p.ok } else { p.text_3 },
    );
    draw_text_weighted(
        cx,
        tr(surface.locale, "home.mcp.title"),
        SANS,
        14.0,
        p.text,
        600,
        Point2D::new(header.origin.x + 15.0, baseline(header, 14.0)),
    );
    if running {
        let state = tr(surface.locale, "home.mcp.running");
        let w = cx.backend.measure_text_family(state, 12.0, SANS);
        draw_text(
            cx,
            state,
            SANS,
            12.0,
            p.text_3,
            Point2D::new(header.origin.x + header.size.x - w, baseline(header, 12.0)),
        );
    } else {
        let start = layout.start;
        let hovered = hover == Some(HomeHit::McpStart);
        cx.backend.fill_round_rect(
            start,
            7.0,
            if hovered {
                super::fade(p.accent, 0.88)
            } else {
                p.accent
            },
        );
        draw_text_weighted(
            cx,
            tr(surface.locale, "home.mcp.start"),
            SANS,
            11.5,
            p.on_accent,
            600,
            Point2D::new(start.origin.x + 10.0, baseline(start, 11.5)),
        );
    }

    for (i, line) in layout.intro_lines.iter().enumerate() {
        draw_text(
            cx,
            line,
            SANS,
            INTRO_FONT,
            p.text_2,
            Point2D::new(
                header.origin.x,
                layout.intro_y + i as f32 * INTRO_LINE + 13.0,
            ),
        );
    }

    caption(
        cx,
        surface,
        tr(surface.locale, "home.mcp.endpoint"),
        header.origin.x,
        layout.endpoint_label_y + 11.0,
    );
    code_box(cx, surface, layout.endpoint_box);
    draw_text(
        cx,
        &local_mcp_url(surface.mcp.port),
        MONO,
        SNIPPET_FONT,
        p.text,
        Point2D::new(
            layout.endpoint_box.origin.x + 12.0,
            baseline(layout.endpoint_box, SNIPPET_FONT),
        ),
    );
    copy_button(cx, surface, layout.copy_endpoint, HomeCopyTarget::Endpoint);

    caption(
        cx,
        surface,
        tr(surface.locale, "home.mcp.agent"),
        header.origin.x,
        layout.agent_label_y + 11.0,
    );
    paint_tabs(surface, cx, layout);
    paint_auto_box(surface, cx, layout);

    draw_text(
        cx,
        tr(surface.locale, "home.mcp.manual"),
        SANS,
        11.0,
        p.text_3,
        Point2D::new(header.origin.x, layout.manual_label_y + 11.0),
    );
    code_box(cx, surface, layout.snippet_box);
    for (i, line) in layout.snippet_lines.iter().enumerate() {
        draw_text(
            cx,
            line,
            MONO,
            SNIPPET_FONT,
            p.text,
            Point2D::new(
                layout.snippet_box.origin.x + 12.0,
                layout.snippet_box.origin.y + 8.0 + i as f32 * SNIPPET_LINE + 12.5,
            ),
        );
    }
    copy_button(cx, surface, layout.copy_snippet, HomeCopyTarget::Snippet);

    cx.backend.fill_rect(
        Rect::xywh(header.origin.x, layout.divider_y, header.size.x, 1.0),
        p.border,
    );
    caption(
        cx,
        surface,
        tr(surface.locale, "home.mcp.connectedHeader"),
        header.origin.x,
        layout.connected_label_y + 11.0,
    );
    let row = layout.connected_row;
    match &surface.mcp.client {
        Some(name) => {
            cx.backend.fill_oval(
                Rect::xywh(row.origin.x + 3.0, row.origin.y + 5.5, 7.0, 7.0),
                p.ok,
            );
            draw_text(
                cx,
                name,
                SANS,
                12.0,
                p.text,
                Point2D::new(row.origin.x + 22.0, baseline(row, 12.0)),
            );
        }
        None => {
            draw_glyph(
                cx.backend,
                HomeGlyph::Plug,
                Point2D::new(row.origin.x, row.origin.y + 2.0),
                14.0,
                p.text_3,
                1.6,
            );
            draw_text(
                cx,
                tr(surface.locale, "home.mcp.emptyAgents"),
                SANS,
                12.0,
                p.text_2,
                Point2D::new(row.origin.x + 22.0, baseline(row, 12.0)),
            );
        }
    }

    let footer = layout.footer;
    draw_text(
        cx,
        &tr(surface.locale, "home.mcp.port").replace("{{port}}", &surface.mcp.port.to_string()),
        SANS,
        12.0,
        p.text_3,
        Point2D::new(footer.origin.x, baseline(footer, 12.0)),
    );
    let link = layout.settings_link;
    let link_color = if hover == Some(HomeHit::McpSettings) {
        p.text
    } else {
        p.accent
    };
    draw_text(
        cx,
        tr(surface.locale, "home.mcp.settings"),
        SANS,
        12.0,
        link_color,
        Point2D::new(link.origin.x, baseline(link, 12.0)),
    );
    draw_icon(
        cx.backend,
        Icon::ArrowRight,
        Point2D::new(link.origin.x + link.size.x - 12.0, link.origin.y + 7.0),
        12.0,
        link_color,
        1.8,
    );

    if surface.state.mcp_agent_menu_open {
        paint_more_menu(surface, cx, layout);
    }
}

fn code_box(cx: &mut PaintCx<'_>, surface: &HomeSurface<'_>, rect: Rect) {
    let p = surface.palette;
    cx.backend.fill_round_rect(rect, 8.0, p.bg);
    cx.backend.stroke_round_rect(rect, 8.0, p.border, 1.0);
}

fn copy_button(
    cx: &mut PaintCx<'_>,
    surface: &HomeSurface<'_>,
    rect: Rect,
    target: HomeCopyTarget,
) {
    let p = surface.palette;
    let hit = match target {
        HomeCopyTarget::Endpoint => HomeHit::McpCopyEndpoint,
        HomeCopyTarget::Snippet => HomeHit::McpCopySnippet,
    };
    let copied = surface.state.copied_recently(target, surface.now_ms);
    let hovered = surface.state.hover == Some(hit);
    cx.backend
        .fill_round_rect(rect, 6.0, if hovered { p.subtle } else { p.surface });
    cx.backend.stroke_round_rect(rect, 6.0, p.border, 1.0);
    draw_icon(
        cx.backend,
        if copied { Icon::Check } else { Icon::Copy },
        Point2D::new(rect.origin.x + 8.0, rect.origin.y + 7.0),
        12.0,
        if copied { p.ok } else { p.text_2 },
        1.7,
    );
    draw_text(
        cx,
        tr(
            surface.locale,
            if copied {
                "home.mcp.copied"
            } else {
                "home.mcp.copy"
            },
        ),
        SANS,
        11.0,
        p.text_2,
        Point2D::new(rect.origin.x + 25.0, baseline(rect, 11.0)),
    );
}

fn paint_tabs(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &McpPopoverLayout) {
    let p = surface.palette;
    code_box(cx, surface, layout.tabs_box);
    for (cli, rect) in &layout.tabs {
        let selected = surface.mcp.agent == *cli;
        let hovered = surface.state.hover == Some(HomeHit::McpAgent(*cli));
        if selected {
            cx.backend.fill_round_rect(*rect, 6.0, p.subtle);
            cx.backend.stroke_round_rect(*rect, 6.0, p.border, 1.0);
        }
        draw_text_weighted(
            cx,
            home_mcp_agent_name(*cli),
            SANS,
            TAB_FONT,
            if selected || hovered {
                p.text
            } else {
                p.text_2
            },
            if selected { 600 } else { 400 },
            Point2D::new(rect.origin.x + 9.0, baseline(*rect, TAB_FONT)),
        );
    }
    let more = layout.more_button;
    let more_selected = !HOME_MCP_TABS.contains(&surface.mcp.agent);
    if more_selected || surface.state.mcp_agent_menu_open {
        cx.backend.fill_round_rect(more, 6.0, p.subtle);
        cx.backend.stroke_round_rect(more, 6.0, p.border, 1.0);
    }
    if more_selected {
        draw_text_weighted(
            cx,
            home_mcp_agent_name(surface.mcp.agent),
            SANS,
            TAB_FONT,
            p.text,
            600,
            Point2D::new(more.origin.x + 9.0, baseline(more, TAB_FONT)),
        );
    }
    draw_icon(
        cx.backend,
        Icon::ChevronDown,
        Point2D::new(more.origin.x + more.size.x - 23.5, more.origin.y + 6.5),
        13.0,
        p.text_2,
        1.6,
    );
}

fn paint_auto_box(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &McpPopoverLayout) {
    let p = surface.palette;
    let rect = layout.auto_box;
    code_box(cx, surface, rect);
    let x = rect.origin.x + 12.0;
    let title = agent_title(surface);
    draw_text_weighted(
        cx,
        &title,
        SANS,
        13.0,
        p.text,
        600,
        Point2D::new(x, rect.origin.y + 24.0),
    );
    if surface.mcp.agent_enabled {
        let title_w = cx.backend.measure_text_family(&title, 13.0, SANS);
        draw_icon(
            cx.backend,
            Icon::Check,
            Point2D::new(x + title_w + 8.0, rect.origin.y + 13.0),
            13.0,
            p.ok,
            2.0,
        );
        draw_text_weighted(
            cx,
            tr(surface.locale, "home.mcp.added"),
            SANS,
            12.0,
            p.ok,
            600,
            Point2D::new(x + title_w + 25.0, rect.origin.y + 24.0),
        );
    }
    draw_text(
        cx,
        &tr(surface.locale, "home.mcp.writes")
            .replace("{{path}}", home_mcp_config_path(surface.mcp.agent)),
        MONO,
        10.5,
        p.text_3,
        Point2D::new(x, rect.origin.y + 42.0),
    );
    let action = layout.action;
    let hovered = surface.state.hover == Some(layout.action_hit);
    let enabled = surface.mcp.agent_enabled;
    if enabled {
        cx.backend
            .fill_round_rect(action, 7.0, if hovered { p.subtle } else { p.surface });
        cx.backend.stroke_round_rect(action, 7.0, p.border, 1.0);
    } else {
        cx.backend.fill_round_rect(
            action,
            7.0,
            if hovered {
                super::fade(p.accent, 0.88)
            } else {
                p.accent
            },
        );
    }
    let ink = if enabled { p.text_2 } else { p.on_accent };
    draw_glyph(
        cx.backend,
        if enabled {
            HomeGlyph::Shared(Icon::Close)
        } else {
            HomeGlyph::Plug
        },
        Point2D::new(action.origin.x + 12.0, action.origin.y + 8.5),
        13.0,
        ink,
        1.7,
    );
    draw_text_weighted(
        cx,
        action_label(surface),
        SANS,
        12.0,
        ink,
        600,
        Point2D::new(action.origin.x + 31.0, baseline(action, 12.0)),
    );
}

fn paint_more_menu(surface: &HomeSurface<'_>, cx: &mut PaintCx<'_>, layout: &McpPopoverLayout) {
    let p = surface.palette;
    paint_popover_card(cx, surface, layout.more_menu);
    for (cli, rect) in &layout.more_rows {
        let selected = surface.mcp.agent == *cli;
        let hovered = surface.state.hover == Some(HomeHit::McpAgent(*cli));
        if selected || hovered {
            cx.backend.fill_round_rect(*rect, 6.0, p.subtle);
        }
        draw_text(
            cx,
            home_mcp_agent_name(*cli),
            SANS,
            12.5,
            if selected { p.text } else { p.text_2 },
            Point2D::new(rect.origin.x + 10.0, baseline(*rect, 12.5)),
        );
        let enabled = surface.ui.agent_settings.mcp_cli_enabled[cli.index()];
        if selected || enabled {
            draw_icon(
                cx.backend,
                Icon::Check,
                Point2D::new(rect.origin.x + rect.size.x - 22.0, rect.origin.y + 8.5),
                13.0,
                if enabled { p.ok } else { p.accent },
                2.0,
            );
        }
    }
}
