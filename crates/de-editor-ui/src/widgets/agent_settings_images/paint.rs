//! Paint pass for the Images settings tab. Geometry comes exclusively from
//! `layout` so the touch paint and hit regions stay aligned.

use super::*;
use crate::widgets::text_metrics;

pub(in crate::widgets) fn paint_images_tab(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
) {
    paint_search_header(cx, theme, settings, ui, content);
    paint_advanced(cx, theme, settings, ui, content, now_ms);
    paint_generation(cx, theme, settings, ui, content, now_ms);
}

fn paint_search_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
) {
    let title_font = 15.0;
    let title_str = t_settings(ui, "settings.images.search");
    let title = TextLayout::single_run(
        title_str,
        "system-ui",
        title_font,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    let title_y = content.origin.y + 20.0;
    cx.backend
        .draw_text(&title, Point2D::new(content.origin.x, title_y));

    let title_w = text_metrics::measure_chrome(cx.backend, title_str, title_font);
    let dot_size = 8.0;
    let dot = Rect::xywh(
        content.origin.x + title_w + 14.0,
        content.origin.y + 11.0,
        dot_size,
        dot_size,
    );
    cx.backend.fill_oval(
        dot,
        if settings.images_search_ready {
            theme.status_success
        } else {
            theme.muted_foreground
        },
    );

    let status_font = 12.0;
    let status_text = if settings.images_search_ready {
        t_settings(ui, "settings.images.ready")
    } else {
        t_settings(ui, "settings.images.notConfigured")
    };
    let status = TextLayout::single_run(
        status_text,
        "system-ui",
        status_font,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &status,
        Point2D::new(content.origin.x + title_w + 30.0, content.origin.y + 20.0),
    );
}

fn paint_advanced(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
) {
    let toggle = advanced_toggle_rect_for_ui(content);
    let icon_size = 14.0;
    draw_icon(
        cx.backend,
        if settings.images_advanced_open {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        Point2D::new(
            toggle.origin.x,
            toggle.origin.y + (toggle.size.y - icon_size) / 2.0,
        ),
        icon_size,
        theme.muted_foreground,
        1.8,
    );
    let label_font = 13.0;
    let advanced = TextLayout::single_run(
        t_settings(ui, "settings.images.advanced"),
        "system-ui",
        label_font,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &advanced,
        Point2D::new(toggle.origin.x + 22.0, toggle.origin.y + 17.0),
    );
    if !settings.images_advanced_open {
        return;
    }

    let subtitle_h = 22.0;
    let subtitle = Rect::xywh(
        content.origin.x,
        toggle.origin.y + toggle.size.y,
        content.size.x,
        subtitle_h,
    );
    let subtitle_font = 12.0;
    let oauth = TextLayout::single_run(
        t_settings(ui, "settings.images.oauthLabel"),
        "system-ui",
        subtitle_font,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &oauth,
        Point2D::new(content.origin.x, subtitle.origin.y + 14.0),
    );

    for (index, field, label_key, placeholder_key) in [
        (
            0,
            ImageSearchField::ClientId,
            "settings.images.clientId",
            "settings.images.clientIdPlaceholder",
        ),
        (
            1,
            ImageSearchField::ClientSecret,
            "settings.images.clientSecret",
            "settings.images.clientSecretPlaceholder",
        ),
    ] {
        paint_search_input_row(
            cx,
            theme,
            settings,
            ui,
            field,
            t_settings(ui, label_key),
            t_settings(ui, placeholder_key),
            search_label_rect_for_ui(content, index),
            search_field_rect_for_ui(content, index),
            now_ms,
        );
    }

    paint_register_link(cx, theme, settings, ui, content);
    paint_search_test(cx, theme, settings, ui, content);
}

fn paint_register_link(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
) {
    let rect = register_link_rect_for_ui(content);
    let font = 12.0;
    let icon_size = 14.0;
    let text = ellipsize(
        cx,
        t_settings(ui, "settings.images.registerLink"),
        (rect.size.x - icon_size - 10.0).max(44.0),
        font,
    );
    let layout = TextLayout::single_run(
        &text,
        "system-ui",
        font,
        theme.primary.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    let baseline = rect.origin.y + 22.0;
    cx.backend
        .draw_text(&layout, Point2D::new(rect.origin.x, baseline));
    let text_w = text_metrics::measure_chrome(cx.backend, &text, font);
    if settings.hover_image_search_register_link {
        cx.backend.fill_rect(
            Rect::xywh(
                rect.origin.x,
                rect.origin.y + rect.size.y - 10.0,
                text_w + 20.0,
                1.0,
            ),
            theme.primary,
        );
    }
    draw_icon(
        cx.backend,
        Icon::ArrowUpRight,
        Point2D::new(
            rect.origin.x + text_w + 6.0,
            rect.origin.y + (rect.size.y - icon_size) / 2.0,
        ),
        icon_size,
        theme.primary,
        1.6,
    );
}

fn paint_search_test(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
) {
    let btn = test_btn_rect_for_ui(content, settings);
    paint_search_test_status(cx, theme, settings, btn);
    let radius = 6.0;
    cx.backend.fill_round_rect(btn, radius, theme.muted);
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        btn,
        settings.hover_image_search_test_button,
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageSearchTest,
        )),
    );
    cx.backend.stroke_round_rect(btn, radius, theme.border, 1.0);
    let font = 13.0;
    let text = t_settings(ui, "settings.images.test");
    let text_w = text_metrics::measure_chrome(cx.backend, text, font);
    let layout = TextLayout::single_run(
        text,
        "system-ui",
        font,
        if search_test_enabled(settings) {
            theme.foreground
        } else {
            theme.muted_foreground
        }
        .to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            btn.origin.x + (btn.size.x - text_w) / 2.0,
            btn.origin.y + btn.size.y / 2.0 + 5.0,
        ),
    );
}

fn paint_generation(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
    now_ms: u64,
) {
    let top = image_gen_section_top_for_ui(content, settings);
    let title_font = 15.0;
    let title = TextLayout::single_run(
        t_settings(ui, "settings.images.generation"),
        "system-ui",
        title_font,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&title, Point2D::new(content.origin.x, top + 20.0));
    paint_add_button(cx, theme, settings, ui, content);

    if settings.image_gen_profiles.is_empty() {
        let font = 13.0;
        let hint = ellipsize(
            cx,
            t_settings(ui, "settings.images.empty"),
            (content.size.x - 24.0).max(44.0),
            font,
        );
        let hint_w = text_metrics::measure_chrome(cx.backend, &hint, font);
        let layout = TextLayout::single_run(
            &hint,
            "system-ui",
            font,
            theme.muted_foreground.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(
            &layout,
            Point2D::new(
                content.origin.x + (content.size.x - hint_w) / 2.0,
                top + DESKTOP_SECTION_TITLE_H + 8.0 + 48.0,
            ),
        );
        return;
    }

    for (index, profile) in settings.image_gen_profiles.iter().enumerate() {
        paint_profile_row(
            cx,
            theme,
            settings,
            ui,
            profile,
            index,
            profile_row_rect_for_ui(content, settings, index),
            now_ms,
        );
    }
}

fn paint_add_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    content: Rect,
) {
    let btn = add_btn_rect_for_ui(content, settings);
    let radius = 6.0;
    cx.backend.fill_round_rect(btn, radius, theme.muted);
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        btn,
        settings.hover_image_gen_add_button,
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageGenAdd,
        )),
    );
    cx.backend.stroke_round_rect(btn, radius, theme.border, 1.0);
    let font = 13.0;
    let text = ellipsize(
        cx,
        t_settings(ui, "settings.images.add"),
        btn.size.x - 8.0,
        font,
    );
    let text_w = text_metrics::measure_chrome(cx.backend, &text, font);
    let layout = TextLayout::single_run(
        &text,
        "system-ui",
        font,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(
            btn.origin.x + (btn.size.x - text_w) / 2.0,
            btn.origin.y + btn.size.y / 2.0 + 5.0,
        ),
    );
}

fn paint_search_test_status(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    btn: Rect,
) {
    let icon_size = 11.0;
    let icon_y = btn.origin.y + (btn.size.y - icon_size) / 2.0;
    let icon_x = btn.origin.x - 20.0;
    match settings.images_search_test_status {
        ImageTestStatus::Idle => {}
        ImageTestStatus::Testing | ImageTestStatus::Valid => draw_icon(
            cx.backend,
            if settings.images_search_test_status == ImageTestStatus::Testing {
                Icon::Loader
            } else {
                Icon::Check
            },
            Point2D::new(icon_x, icon_y),
            icon_size,
            if settings.images_search_test_status == ImageTestStatus::Valid {
                theme.primary
            } else {
                theme.muted_foreground
            },
            if settings.images_search_test_status == ImageTestStatus::Valid {
                1.8
            } else {
                1.5
            },
        ),
        ImageTestStatus::Invalid => paint_invalid(cx, theme, btn),
    }
}

fn paint_invalid(cx: &mut PaintCx<'_>, theme: &Theme, btn: Rect) {
    let font = 10.0;
    let layout = TextLayout::single_run(
        "Invalid",
        "system-ui",
        font,
        theme.destructive.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &layout,
        Point2D::new(btn.origin.x - 44.0, btn.origin.y + 17.0),
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_profile_row(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    profile: &ImageGenProfile,
    index: usize,
    row: Rect,
    now_ms: u64,
) {
    let active = settings.active_image_gen_profile_id.as_deref() == Some(profile.id.as_str());
    let editing = is_editing_profile(settings, index);
    let radius = 6.0;
    if active || editing {
        cx.backend.fill_round_rect(row, radius, theme.muted);
    }
    cx.backend.stroke_round_rect(
        row,
        radius,
        if active { theme.primary } else { theme.border },
        1.0,
    );
    paint_profile_header(
        cx, theme, settings, ui, profile, index, row, active, editing,
    );
    if !editing {
        return;
    }

    for field in image_gen_fields() {
        paint_profile_field(
            cx,
            theme,
            settings,
            ui,
            profile,
            index,
            field,
            profile_input_rect_for_ui(row, field),
            profile_field_label_rect_for_ui(row, profile_field_index(field)),
            now_ms,
        );
    }
    paint_profile_test_button(
        cx,
        theme,
        ui,
        profile,
        profile_test_btn_rect_for_ui(row),
        settings.hover_image_gen_profile_test == Some(index),
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileTest(index),
        )),
    );
    let provider_rect = profile_provider_rect_for_ui(row);
    paint_provider_field(
        cx,
        theme,
        profile,
        provider_rect,
        profile_field_label_rect_for_ui(row, 1),
        t_settings(ui, "builtin.provider"),
        settings.hover_image_gen_profile_provider == Some(index),
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileProvider(index),
        )),
    );
    if settings.image_gen_provider_menu_open == Some(index) {
        let hovered = settings
            .hover_image_gen_provider_option
            .and_then(|(hover_index, provider)| (hover_index == index).then_some(provider));
        let pressed = match ui.pressed_button {
            Some(ButtonPressTarget::AgentSettings(AgentSettingsButton::ImageProviderOption {
                index: pressed_index,
                provider,
            })) if pressed_index == index => Some(provider),
            _ => None,
        };
        paint_provider_menu(cx, theme, provider_rect, profile.provider, hovered, pressed);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_profile_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    settings: &AgentSettings,
    ui: &EditorUiState,
    profile: &ImageGenProfile,
    index: usize,
    row: Rect,
    active: bool,
    editing: bool,
) {
    let header = profile_header_rect_for_ui(row);
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        header,
        settings.hover_image_gen_profile_header == Some(index),
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileHeader(index),
        )),
    );
    let dot = profile_active_indicator_rect(row);
    if active {
        cx.backend.fill_oval(dot, theme.primary);
        let check = 8.0;
        draw_icon(
            cx.backend,
            Icon::Check,
            Point2D::new(
                dot.origin.x + (dot.size.x - check) / 2.0,
                dot.origin.y + (dot.size.y - check) / 2.0,
            ),
            check,
            theme.primary_foreground,
            2.0,
        );
    } else {
        cx.backend.stroke_oval(dot, theme.muted_foreground, 1.5);
    }

    let name_font = 12.0;
    let raw_name = if profile.name.trim().is_empty() {
        profile.provider.label()
    } else {
        profile.name.as_str()
    };
    let name = ellipsize(cx, raw_name, row.size.x - 180.0, name_font);
    let name_layout = TextLayout::single_run(
        &name,
        "system-ui",
        name_font,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &name_layout,
        Point2D::new(row.origin.x + 32.0, row.origin.y + 20.0),
    );

    let remove = profile_remove_hover_rect_for_ui(row);
    let chevron = profile_chevron_rect_for_ui(row);
    let provider_font = 10.0;
    let provider_w =
        text_metrics::measure_chrome(cx.backend, profile.provider.label(), provider_font);
    let provider_layout = TextLayout::single_run(
        profile.provider.label(),
        "system-ui",
        provider_font,
        theme.muted_foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend.draw_text(
        &provider_layout,
        Point2D::new(chevron.origin.x - 12.0 - provider_w, row.origin.y + 20.0),
    );
    let chev_size = 12.0;
    draw_icon(
        cx.backend,
        if editing {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        },
        Point2D::new(
            chevron.origin.x + 4.0,
            chevron.origin.y + (chevron.size.y - chev_size) / 2.0,
        ),
        chev_size,
        theme.muted_foreground,
        1.5,
    );
    crate::widgets::button::paint_ghost_button_feedback(
        cx.backend,
        theme,
        remove,
        settings.hover_image_gen_profile_remove == Some(index),
        ui.button_pressed(ButtonPressTarget::AgentSettings(
            AgentSettingsButton::ImageProfileRemove(index),
        )),
    );
    let trash = 12.0;
    draw_icon(
        cx.backend,
        Icon::Trash,
        Point2D::new(
            remove.origin.x + (remove.size.x - trash) / 2.0,
            remove.origin.y + (remove.size.y - trash) / 2.0,
        ),
        trash,
        theme.muted_foreground,
        1.5,
    );
}
