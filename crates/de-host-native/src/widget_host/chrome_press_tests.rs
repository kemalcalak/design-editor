//! Presses on the floating chrome (file pill + canvas action group) and
//! on the File menu's app-level tail, through the real press path.

use super::WidgetHostNative;
use de_editor_core::editor_ui_state::FileAction;
use de_editor_core::{ChromeButton, PropertyTab};
use de_editor_ui::widgets::chrome_tooltip::button_anchor;
use de_editor_ui::widgets::file_menu::{FileMenu, FileMenuChoice, FileMenuTool, MenuHit};
use de_editor_ui::widgets::host_overlay_geometry::file_menu_anchor;
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1400.0;
const VIEWPORT_H: f32 = 900.0;

fn hosted() -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    // Stay on the canvas so the Home surface does not swallow the press.
    host.editor_state.editor_ui.home.hide();
    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    host
}

fn press_button(host: &mut WidgetHostNative, button: ChromeButton) {
    let rect = button_anchor(&host.editor_state, VIEWPORT_W, VIEWPORT_H, button)
        .expect("button paints in this build");
    let x = rect.origin.x + rect.size.x / 2.0;
    let y = rect.origin.y + rect.size.y / 2.0;
    assert!(host.apply_press(x, y, VIEWPORT_W, VIEWPORT_H));
    host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H);
}

#[test]
fn pressing_home_queues_file_action_home() {
    let mut host = hosted();
    press_button(&mut host, ChromeButton::Home);
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::Home)
    );
}

#[test]
fn pressing_the_save_state_saves() {
    let mut host = hosted();
    press_button(&mut host, ChromeButton::Save);
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::Save)
    );
}

#[test]
fn pressing_export_opens_the_export_dialog_path() {
    let mut host = hosted();
    press_button(&mut host, ChromeButton::OpenExportDialog);
    assert_eq!(
        host.editor_state.editor_ui.pending_file_action,
        Some(FileAction::ExportImage)
    );
}

#[test]
fn the_code_button_flips_the_right_panel_to_code_and_back() {
    let mut host = hosted();
    press_button(&mut host, ChromeButton::ToggleCodeView);
    assert_eq!(host.editor_state.editor_ui.property_tab, PropertyTab::Code);
    press_button(&mut host, ChromeButton::ToggleCodeView);
    assert_eq!(
        host.editor_state.editor_ui.property_tab,
        PropertyTab::Design
    );
}

#[test]
fn the_file_name_opens_the_file_menu() {
    let mut host = hosted();
    press_button(&mut host, ChromeButton::ToggleFileMenu);
    assert!(host.editor_state.editor_ui.file_menu_open);
}

/// Press the File menu row (or footer tool) that resolves to `wanted`.
fn press_menu_choice(host: &mut WidgetHostNative, wanted: FileMenuChoice) {
    host.editor_state.editor_ui.file_menu_open = true;
    let menu = FileMenu::from_editor_ui(&host.editor_state.editor_ui, 0);
    let panel = menu.rect_at(file_menu_anchor(&host.editor_state, VIEWPORT_W, VIEWPORT_H));
    let point = (0..panel.size.y as i32)
        .flat_map(|dy| (0..panel.size.x as i32).step_by(4).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| Point2D::new(panel.origin.x + dx as f32, panel.origin.y + dy as f32))
        .find(|p| match menu.hit(panel, *p) {
            MenuHit::Row(row) => menu.choice_for_row(row) == Some(wanted),
            _ => false,
        })
        .expect("the menu offers the choice");
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
}

#[test]
fn the_menu_footer_flips_the_theme_and_closes() {
    let mut host = hosted();
    let before = host.editor_state.editor_ui.theme_mode;
    press_menu_choice(&mut host, FileMenuChoice::Tool(FileMenuTool::ToggleTheme));
    assert_ne!(host.editor_state.editor_ui.theme_mode, before);
    assert!(!host.editor_state.editor_ui.file_menu_open);
}

#[test]
fn the_menu_footer_hides_the_left_panel() {
    let mut host = hosted();
    assert!(host.editor_state.editor_ui.sidebar_open);
    press_menu_choice(&mut host, FileMenuChoice::Tool(FileMenuTool::ToggleSidebar));
    assert!(!host.editor_state.editor_ui.sidebar_open);
}

#[test]
fn the_menu_footer_opens_settings_and_git() {
    let mut host = hosted();
    press_menu_choice(&mut host, FileMenuChoice::Tool(FileMenuTool::Settings));
    assert!(host.editor_state.editor_ui.agent_settings_open);

    let mut host = hosted();
    press_menu_choice(&mut host, FileMenuChoice::Tool(FileMenuTool::Git));
    assert!(host.editor_state.editor_ui.git_panel.open);
}

#[test]
fn the_import_row_opens_the_import_menu() {
    let mut host = hosted();
    press_menu_choice(&mut host, FileMenuChoice::Import);
    assert!(host.editor_state.editor_ui.import_menu_open);
    assert!(!host.editor_state.editor_ui.file_menu_open);
}
