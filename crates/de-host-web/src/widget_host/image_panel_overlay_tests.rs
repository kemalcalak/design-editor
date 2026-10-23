use super::WidgetHost;
use de_editor_core::{EditorState, NodeId, SettingsFocus};
use de_editor_ui::widgets::file_menu::{FileMenu, FileMenuChoice, FileMenuTool, MenuHit};
use de_editor_ui::widgets::{LayerPanel, LayerPanelHit};
use de_editor_ui::Point2D;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

#[test]
fn settings_from_the_file_menu_take_input_ownership_from_image_search() {
    let mut host = WidgetHost::new();
    let _ = host
        .editor_state
        .insert_image_node_at_viewport("Hero photo", "https://x/y.png");
    host.editor_state.editor_ui.image_panel.search_open = true;
    host.editor_state
        .editor_ui
        .image_panel
        .search_query
        .set_text("hero query");

    host.last_viewport_w = VIEWPORT_W;
    host.last_viewport_h = VIEWPORT_H;
    // The file pill opens the menu — a higher overlay, so the image
    // popover closes on that first press.
    let pill = de_editor_ui::widgets::chrome_tooltip::button_anchor(
        &host.editor_state,
        VIEWPORT_W,
        VIEWPORT_H,
        de_editor_core::ChromeButton::ToggleFileMenu,
    )
    .expect("file pill paints");
    assert!(host.apply_press(
        pill.origin.x + pill.size.x / 2.0,
        pill.origin.y + pill.size.y / 2.0,
        VIEWPORT_W,
        VIEWPORT_H
    ));
    assert!(!host.editor_state.editor_ui.image_panel.search_open);

    let menu = FileMenu::from_editor_ui(&host.editor_state.editor_ui, 0);
    let panel = menu.rect_at(
        de_editor_ui::widgets::host_overlay_geometry::file_menu_anchor(
            &host.editor_state,
            VIEWPORT_W,
            VIEWPORT_H,
        ),
    );
    let wanted = FileMenuChoice::Tool(FileMenuTool::Settings);
    let point = (0..panel.size.y as i32)
        .flat_map(|dy| (0..panel.size.x as i32).step_by(2).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| Point2D::new(panel.origin.x + dx as f32, panel.origin.y + dy as f32))
        .find(|p| match menu.hit(panel, *p) {
            MenuHit::Row(row) => menu.choice_for_row(row) == Some(wanted),
            _ => false,
        })
        .expect("the footer offers Settings");
    assert!(host.apply_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.editor_state.editor_ui.agent_settings_open);

    host.editor_state.editor_ui.agent_settings.focus = Some(SettingsFocus::McpPort);
    host.editor_state.editor_ui.settings_input.set_text("123");
    assert!(host.apply_paste_text("4"));
    assert_eq!(host.editor_state.editor_ui.settings_input.text(), "1234");
    assert_eq!(
        host.editor_state.editor_ui.image_panel.search_query.text(),
        "hero query"
    );
}

#[test]
fn selection_changing_right_press_closes_image_search() {
    let doc = jian_ops_schema::load_str(
        r#"{"version":"1.0.0","children":[
          {"type":"rectangle","id":"n1","name":"Other","x":0,"y":0,"width":100,"height":50},
          {"type":"image","id":"n2","name":"Hero","x":120,"y":0,"width":100,"height":100,"src":"https://x/y.png"}
        ]}"#,
    )
    .expect("fixture")
    .value;
    let mut host = WidgetHost::new();
    host.editor_state = EditorState::from_document(doc);
    host.editor_state.set_single_selection(NodeId::new("n2"));
    host.editor_state.editor_ui.image_panel.search_open = true;
    host.editor_state
        .editor_ui
        .image_panel
        .search_query
        .set_text("hero query");

    let rect = host.layer_panel_rect(VIEWPORT_H);
    let panel = LayerPanel::from_editor(&host.editor_state);
    let mut point = None;
    let mut y = rect.origin.y;
    while y < rect.origin.y + rect.size.y {
        let candidate = Point2D::new(48.0, y);
        if panel.hit_test(rect, candidate) == Some(LayerPanelHit::Layer(NodeId::new("n1"))) {
            point = Some(candidate);
            break;
        }
        y += 1.0;
    }
    let point = point.expect("other layer row");
    assert!(host.apply_right_press(point.x, point.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(host.editor_state.selection.anchor, NodeId::new("n1"));
    assert!(!host.editor_state.editor_ui.image_panel.search_open);
    assert!(!host.apply_text('x'));
    assert_eq!(
        host.editor_state.editor_ui.image_panel.search_query.text(),
        "hero query"
    );
}

#[test]
fn image_popover_edges_preserve_utf8_and_shift_selection() {
    let mut host = WidgetHost::new();
    let panel = &mut host.editor_state.editor_ui.image_panel;
    panel.search_open = true;
    panel.search_query.set_text("a你bc");
    panel.search_query.set_caret(1, 0);

    assert!(host.apply_image_panel_edge(true, false));
    assert_eq!(
        host.editor_state.editor_ui.image_panel.search_query.caret(),
        "a你bc".len()
    );
    assert!(host.apply_image_panel_edge(false, true));
    assert_eq!(
        host.editor_state
            .editor_ui
            .image_panel
            .search_query
            .highlight_range(),
        Some((0, "a你bc".len()))
    );
}
