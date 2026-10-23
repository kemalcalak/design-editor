//! Right-rail Code tab: framework select, generate/cancel phases, copy,
//! preview drag + wheel routing, and the import-menu / rail-routing tails.
//!
//! Split out of `input_tests.rs` to keep every file under the repo's
//! 800-line cap.

use super::*;

#[test]
fn codegen_select_framework_updates_state() {
    use de_editor_core::codegen::Framework;
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::PropertyPanelAction;
    let mut host = WidgetHostNative::new();

    host.apply_property_action(PropertyPanelAction::Codegen(
        CodegenAction::SelectFramework(Framework::Vue),
    ));

    assert_eq!(host.editor_state().codegen.framework, Framework::Vue);
}

#[test]
fn codegen_generate_raises_pending_and_generating_phase() {
    use de_editor_core::codegen::CodegenPhase;
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::PropertyPanelAction;
    let mut host = WidgetHostNative::new();

    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Generate));

    assert!(host.editor_state().codegen.pending_generate);
    assert_eq!(host.editor_state().codegen.phase, CodegenPhase::Generating);
}

#[test]
fn codegen_copy_queues_code_to_system_clipboard() {
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::PropertyPanelAction;
    let mut host = WidgetHostNative::new();
    host.set_now_ms(4242);
    host.editor_state_mut().codegen.code = "export const App = () => null;".to_string();

    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Copy));

    assert_eq!(host.editor_state().codegen.copied_at, Some(4242));
    assert_eq!(
        host.editor_state().chat.pending_copy_text.as_deref(),
        Some("export const App = () => null;"),
    );
}

#[test]
fn codegen_copy_button_queues_full_code_even_with_selection() {
    use de_editor_core::codegen::CodeSelection;
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::PropertyPanelAction;
    let mut host = WidgetHostNative::new();
    host.set_now_ms(4242);
    host.editor_state_mut().codegen.code = "export const App = () => null;".to_string();
    host.editor_state_mut().codegen.code_selection = Some(CodeSelection {
        anchor: 7,
        focus: 12,
    });

    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Copy));

    assert_eq!(host.editor_state().codegen.copied_at, Some(4242));
    assert_eq!(
        host.editor_state().chat.pending_copy_text.as_deref(),
        Some("export const App = () => null;"),
    );
}

#[test]
fn shortcut_copy_prefers_selected_code_text() {
    use de_editor_core::codegen::CodeSelection;
    let mut host = WidgetHostNative::new();
    host.editor_state_mut().codegen.code = "export const App = () => null;".to_string();
    host.editor_state_mut().codegen.code_selection = Some(CodeSelection {
        anchor: 7,
        focus: 12,
    });

    assert!(host.apply_copy());
    assert_eq!(
        host.editor_state().chat.pending_copy_text.as_deref(),
        Some("const"),
    );
}

#[test]
fn codegen_hover_tracks_idle_generate_button() {
    use de_editor_core::codegen::CodegenHover;
    use de_editor_core::PropertyTab;
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::property_panel_code;
    use de_editor_ui::widgets::property_panel_inputs::PANEL_HEADER_HEIGHT;
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"n-code","name":"n-code","x":0,"y":0,"width":100,"height":50}]}"#,
    );
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n-code"));
    host.editor_state_mut().editor_ui.property_tab = PropertyTab::Code;

    let pw = host.editor_state().editor_ui.property_panel_width;
    let panel_x = host.last_viewport_w - pw;
    let (_, generate_rect) = property_panel_code::code_action_rects(
        panel_x,
        PROPERTY_PANEL_TOP + PANEL_HEADER_HEIGHT,
        pw,
        &host.editor_state().codegen,
    )
    .into_iter()
    .find(|(action, _)| matches!(action, CodegenAction::Generate))
    .expect("Generate rect present");
    let point = de_editor_ui::Point2D::new(
        generate_rect.origin.x + generate_rect.size.x / 2.0,
        generate_rect.origin.y + generate_rect.size.y / 2.0,
    );

    assert!(host.apply_cursor_move(point.x, point.y));
    assert_eq!(
        host.editor_state().codegen.action_hover,
        Some(CodegenHover::Generate)
    );

    assert!(host.apply_cursor_move(panel_x - 12.0, PROPERTY_PANEL_TOP + PANEL_HEADER_HEIGHT));
    assert_eq!(host.editor_state().codegen.action_hover, None);
}

#[test]
fn codegen_preview_drag_selects_code_text() {
    use de_editor_core::codegen::{CodeSelection, CodegenPhase};
    use de_editor_core::PropertyTab;
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"n-code","name":"n-code","x":0,"y":0,"width":100,"height":50}]}"#,
    );
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n-code"));
    host.editor_state_mut().editor_ui.property_tab = PropertyTab::Code;
    host.editor_state_mut().codegen.phase = CodegenPhase::Complete;
    host.editor_state_mut().codegen.code = "import React\nconst n = 1".into();

    let panel_x = viewport_w - host.editor_state().editor_ui.property_panel_width;
    let char_w = 11.0 * 0.55;
    let start = de_editor_ui::Point2D::new(panel_x + 66.0, PROPERTY_PANEL_TOP + 112.0);
    let end = de_editor_ui::Point2D::new(panel_x + 66.0 + char_w * 6.0, PROPERTY_PANEL_TOP + 112.0);

    assert!(host.apply_press(start.x, start.y, viewport_w, viewport_h));
    assert!(host.apply_cursor_move(end.x, end.y));
    assert_eq!(
        host.editor_state().codegen.code_selection,
        Some(CodeSelection {
            anchor: 0,
            focus: 6
        })
    );
    assert!(host.apply_release_with_viewport(viewport_w, viewport_h));
}

#[test]
fn codegen_preview_wheel_scrolls_code_not_property_panel() {
    use de_editor_core::codegen::CodegenPhase;
    use de_editor_core::PropertyTab;
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"n-code","name":"n-code","x":0,"y":0,"width":100,"height":50}]}"#,
    );
    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n-code"));
    host.editor_state_mut().editor_ui.property_tab = PropertyTab::Code;
    host.editor_state_mut().codegen.phase = CodegenPhase::Complete;
    host.editor_state_mut().codegen.code = (0..100)
        .map(|i| format!("line-{i:02}"))
        .collect::<Vec<_>>()
        .join("\n");

    let panel_x = viewport_w - host.editor_state().editor_ui.property_panel_width;
    assert!(host.apply_wheel(
        panel_x + 80.0,
        PROPERTY_PANEL_TOP + 112.0,
        -160.0,
        viewport_w,
        viewport_h
    ));

    assert!(host.editor_state().codegen.code_scroll.offset > 0.0);
    assert_eq!(
        host.editor_state().editor_ui.property_panel_scroll.offset,
        0.0
    );
}

#[test]
fn code_view_close_button_tracks_hover() {
    use de_editor_core::PropertyTab;
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"n-tabs","name":"n-tabs","x":0,"y":0,"width":100,"height":50}]}"#,
    );
    host.last_viewport_w = 1200.0;
    host.last_viewport_h = 800.0;
    host.editor_state_mut()
        .set_single_selection(NodeId::new("n-tabs"));
    host.editor_state_mut().editor_ui.property_tab = PropertyTab::Code;

    // The Code view's close button reports the tab it returns to.
    let panel_w = host.editor_state().editor_ui.property_panel_width;
    let panel_x = host.last_viewport_w - panel_w;
    let close = de_editor_ui::widgets::property_panel_header::close_rect(de_editor_ui::Rect::xywh(
        panel_x,
        PROPERTY_PANEL_TOP,
        panel_w,
        host.last_viewport_h,
    ));
    assert!(host.apply_cursor_move(
        close.origin.x + close.size.x / 2.0,
        close.origin.y + close.size.y / 2.0
    ));
    assert!(host.editor_state().editor_ui.property_close_hover);

    assert!(host.apply_cursor_move(panel_x - 12.0, PROPERTY_PANEL_TOP + 18.0));
    assert!(!host.editor_state().editor_ui.property_close_hover);
}

#[test]
fn codegen_cancel_resets_phase_by_code_presence() {
    use de_editor_core::codegen::CodegenPhase;
    use de_editor_ui::widgets::property_panel_action::CodegenAction;
    use de_editor_ui::widgets::PropertyPanelAction;
    let mut host = WidgetHostNative::new();
    // No code yet → Cancel falls back to Idle.
    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Generate));
    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Cancel));
    assert!(!host.editor_state().codegen.pending_generate);
    assert_eq!(host.editor_state().codegen.phase, CodegenPhase::Idle);

    // With code present → Cancel returns to Complete.
    host.editor_state_mut().codegen.code = "rendered".to_string();
    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Generate));
    host.apply_property_action(PropertyPanelAction::Codegen(CodegenAction::Cancel));
    assert_eq!(host.editor_state().codegen.phase, CodegenPhase::Complete);
}

/// File ▸ Import opens a two-row menu; the Figma row keeps the existing
/// modal, and the HTML row raises the file action the desktop runner
/// turns into a file dialog + background import.
#[test]
fn import_menu_routes_its_two_rows_to_figma_and_html() {
    use de_editor_ui::widgets::ImportMenu;

    let (vw, vh) = (1200.0, 800.0);
    let open_menu = || {
        let mut host = WidgetHostNative::new();
        host.last_viewport_h = vh;
        de_editor_core::host_press_transitions::toggle_import_menu(
            &mut host.editor_state_mut().editor_ui,
        );
        host
    };
    let row_point = |host: &WidgetHostNative, idx: usize| {
        let menu = ImportMenu::for_editor_ui(&host.editor_state().editor_ui);
        let (anchor, viewport) = host.import_menu_anchor(vw, vh);
        let panel = menu.popup_rect(anchor, viewport);
        let row_h = panel.size.y / 2.0;
        de_editor_ui::Point2D::new(
            panel.origin.x + panel.size.x / 2.0,
            panel.origin.y + row_h * idx as f32 + row_h / 2.0,
        )
    };

    // Figma row → the existing import modal.
    let mut host = open_menu();
    assert!(host.editor_state().editor_ui.import_menu_open);
    let figma_row = row_point(&host, 0);
    assert!(host.apply_press(figma_row.x, figma_row.y, vw, vh));
    assert!(!host.editor_state().editor_ui.import_menu_open);
    assert!(host.editor_state().editor_ui.figma_import_open);
    assert_eq!(
        host.editor_state().editor_ui.import_source,
        de_editor_core::figma_import_state::ImportSource::Figma
    );
    assert_eq!(host.editor_state().editor_ui.pending_file_action, None);

    // HTML row → the same modal, showing the HTML source.
    let mut host = open_menu();
    let html_row = row_point(&host, 1);
    assert!(host.apply_press(html_row.x, html_row.y, vw, vh));
    assert!(!host.editor_state().editor_ui.import_menu_open);
    assert!(host.editor_state().editor_ui.figma_import_open);
    assert_eq!(
        host.editor_state().editor_ui.import_source,
        de_editor_core::figma_import_state::ImportSource::Html
    );
    assert_eq!(host.editor_state().editor_ui.pending_file_action, None);
}

#[test]
fn right_rail_host_routing_tracks_design_selection_and_code_fallback() {
    let mut host = WidgetHostNative::new();
    seed(
        &mut host,
        r#"{"version":"1.0.0","children":[{"type":"rectangle","id":"n-rail","name":"Rail probe","x":0,"y":0,"width":100,"height":50}]}"#,
    );
    host.editor_state_mut().chat.minimize();
    host.editor_state_mut().editor_ui.property_tab = de_editor_core::PropertyTab::Design;
    host.editor_state_mut().clear_selection();

    let viewport_w = 1200.0;
    let viewport_h = 800.0;
    let property_width = host.editor_state().editor_ui.property_panel_width;
    let old_property_gutter_x = viewport_w - property_width;
    let press_y = PROPERTY_PANEL_TOP + 180.0;

    let (canvas_left, _, canvas_width, _) = host.canvas_region(viewport_w, viewport_h);
    assert_eq!(
        canvas_left + canvas_width,
        viewport_w,
        "empty Design selection must release the entire right rail to the canvas"
    );
    assert_eq!(
        host.panel_resize_hover(old_property_gutter_x, press_y, viewport_w),
        None,
        "the former property-panel gutter must not remain interactive"
    );

    let right_edge_x = viewport_w - 2.0;
    assert!(host.over_canvas(right_edge_x, press_y, viewport_w, viewport_h));
    host.apply_press(right_edge_x, press_y, viewport_w, viewport_h);
    assert!(
        host.marquee_drag.is_some(),
        "a blank press at the viewport's right edge must route to the canvas"
    );
    assert!(!host.is_resizing_panel());
    assert!(host.apply_release_with_viewport(viewport_w, viewport_h));

    host.editor_state_mut()
        .set_single_selection(NodeId::new("n-rail"));
    assert!(host.editor_state().right_rail_visible());
    let (selected_left, _, selected_width, _) = host.canvas_region(viewport_w, viewport_h);
    assert_eq!(selected_left + selected_width, old_property_gutter_x);
    assert!(matches!(
        host.panel_resize_hover(old_property_gutter_x, press_y, viewport_w),
        Some(crate::widget_host::PanelResizeKind::PropertyLeft)
    ));
    assert!(!host.over_canvas(right_edge_x, press_y, viewport_w, viewport_h));

    host.editor_state_mut().clear_selection();
    host.editor_state_mut().editor_ui.property_tab = de_editor_core::PropertyTab::Code;
    assert!(
        host.editor_state().right_rail_visible(),
        "Code remains selection-independent"
    );
    let (code_left, _, code_width, _) = host.canvas_region(viewport_w, viewport_h);
    assert_eq!(code_left + code_width, old_property_gutter_x);
    assert!(matches!(
        host.panel_resize_hover(old_property_gutter_x, press_y, viewport_w),
        Some(crate::widget_host::PanelResizeKind::PropertyLeft)
    ));
}
