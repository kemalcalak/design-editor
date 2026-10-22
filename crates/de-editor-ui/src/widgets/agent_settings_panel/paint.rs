//! Paint pass for [`AgentSettingsPanel`] — the modal frame, close
//! button, Agents tab body, and the shared section-header primitives,
//! plus the `Widget` impl and the drag hook. The top tab strip lives in
//! the `tabs` sibling and the Agents headline in `hero`. Carved off
//! `agent_settings_panel.rs` to keep every file under the 800-line cap.

use super::*;

struct PanelPaintArgs<'a> {
    agents: crate::widgets::agent_settings_agents::AgentsTab<'a>,
    theme: &'a Theme,
    settings: &'a AgentSettings,
    ui: &'a EditorUiState,
    now_ms: u64,
    mode: AgentSettingsPanelMode,
    layout: AgentSettingsLayout,
    scroll: f32,
}

impl<'a> Widget for AgentSettingsPanel<'a> {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout(&self, _cx: &crate::widgets::LayoutCx) -> crate::widgets::LayoutBox {
        crate::widgets::LayoutBox {
            rect: Rect {
                origin: Point2D::new(0.0, 0.0),
                size: Point2D::new(PANEL_WIDTH, PANEL_HEIGHT),
            },
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let layout = self.resolved_layout(rect);
        let scroll = self.effective_scroll(rect);
        paint_panel(
            cx,
            rect,
            PanelPaintArgs {
                agents: self.agents_tab(),
                theme: &self.theme,
                settings: &self.settings,
                ui: self.ui,
                now_ms: self.now_ms,
                mode: self.mode,
                layout,
                scroll,
            },
        );
        if self.mode.active_tab(&self.settings) == AgentSettingsTab::Fonts {
            agent_settings_fonts::paint_picker(
                cx,
                &self.theme,
                rect,
                hero_body_rect_for_ui(layout.content),
                self.ui,
                scroll,
                self.now_ms,
            );
        }
    }

    fn access_node(&self) -> accesskit::Node {
        let mut node = accesskit::Node::new(accesskit::Role::Dialog);
        node.set_label("Settings");
        node
    }
}

fn paint_panel(cx: &mut PaintCx<'_>, panel: Rect, args: PanelPaintArgs<'_>) {
    let PanelPaintArgs {
        agents,
        theme,
        settings,
        ui,
        now_ms,
        mode,
        layout,
        scroll,
    } = args;
    cx.backend.fill_round_rect(panel, 12.0, theme.card);
    // The navigation rail sits on the page background: a rounded
    // fill for the left corners, squared off on its right edge.
    let rail = crate::widgets::agent_settings_panel_geometry::nav_rail_rect(panel);
    cx.backend.fill_round_rect(rail, 12.0, theme.background);
    cx.backend.fill_rect(
        Rect::xywh(
            rail.origin.x + rail.size.x - 12.0,
            rail.origin.y,
            12.0,
            rail.size.y,
        ),
        theme.background,
    );
    cx.backend.fill_rect(
        Rect::xywh(
            rail.origin.x + rail.size.x,
            panel.origin.y,
            1.0,
            panel.size.y,
        ),
        theme.border,
    );
    cx.backend.stroke_round_rect(panel, 12.0, theme.border, 1.0);
    paint_title(cx, theme, ui, layout);
    super::tabs::paint_tab_strip(cx, theme, settings, ui, layout, mode);
    let content_rect = layout.content;
    cx.backend.save();
    cx.backend.clip_rect(layout.content_paint_clip());
    cx.backend.translate(Point2D::new(0.0, -scroll));
    match mode.active_tab(settings) {
        AgentSettingsTab::Agents => {
            super::hero::paint_agents_hero(cx, theme, ui, content_rect);
            agents.paint(
                cx,
                theme,
                super::hit_test::agents_body(content_rect),
                now_ms,
            );
        }
        AgentSettingsTab::Mcp => {
            agent_settings_mcp::paint_mcp_tab(cx, theme, settings, ui, content_rect, now_ms)
        }
        AgentSettingsTab::Images => {
            paint_secondary_hero(cx, theme, ui, content_rect, "settings.images");
            let body_h = agent_settings_images::responsive_content_height(settings);
            crate::widgets::agent_settings_rows::paint_row_card(
                cx,
                theme,
                crate::widgets::agent_settings_panel_geometry::secondary_card_rect(
                    content_rect,
                    body_h,
                ),
            );

            agent_settings_images::paint_images_tab(
                cx,
                theme,
                settings,
                ui,
                hero_body_rect_for_ui(content_rect),
                now_ms,
            )
        }
        AgentSettingsTab::Fonts => {
            paint_secondary_hero(cx, theme, ui, content_rect, "settings.fonts");
            let body_h = agent_settings_fonts::content_height(ui);
            crate::widgets::agent_settings_rows::paint_row_card(
                cx,
                theme,
                crate::widgets::agent_settings_panel_geometry::secondary_card_rect(
                    content_rect,
                    body_h,
                ),
            );

            agent_settings_fonts::paint_fonts_tab(
                cx,
                theme,
                ui,
                hero_body_rect_for_ui(content_rect),
            )
        }
        AgentSettingsTab::System => {
            agent_settings_system::paint_system_tab(cx, theme, settings, ui, content_rect)
        }
    }
    cx.backend.restore();
    paint_close(cx, theme, settings, ui, layout);
}

fn paint_title(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    layout: AgentSettingsLayout,
) {
    // The rail is titled in Home's 15 px semibold.
    let text = TextLayout::single_run(
        t_settings(ui, "settings.title"),
        crate::widgets::agent_settings_rows::SETTINGS_FONT_FAMILY,
        15.0,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    )
    .with_font_weight(600);
    cx.backend.draw_text(
        &text,
        Point2D::new(layout.header.origin.x + 20.0, layout.header.origin.y + 34.0),
    );
}

/// Tab intro for the tabs the panel paints one on behalf of. `prefix`
/// names the i18n family (`settings.images` resolves to
/// `settings.images.heroTitle` plus `.heroSubtitle`), so the key pair
/// can't drift from the tab it belongs to.
fn paint_secondary_hero(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    ui: &EditorUiState,
    content: Rect,
    prefix: &str,
) {
    let (title, subtitle) = match prefix {
        "settings.images" => ("settings.tab.images", "settings.images.heroSubtitle"),
        _ => ("settings.tab.fonts", "settings.fonts.heroSubtitle"),
    };
    crate::widgets::agent_settings_rows::paint_tab_intro(
        cx,
        theme,
        content,
        t_settings(ui, title),
        Some(t_settings(ui, subtitle)),
    );
}

fn paint_close(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    layout: AgentSettingsLayout,
) {
    let pressed = ui.button_pressed(ButtonPressTarget::AgentSettings(AgentSettingsButton::Close));
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        layout.close_target,
        settings.hover_agent_settings_close,
        pressed,
    );
    draw_icon(
        cx.backend,
        Icon::Close,
        layout.close_icon.origin,
        layout.close_icon.size.x,
        theme.foreground,
        2.0,
    );
}

pub fn drag_for_hit(
    _hit: AgentSettingsHit,
) -> Option<de_editor_core::agent_settings::AgentSettingsDrag> {
    None
}
