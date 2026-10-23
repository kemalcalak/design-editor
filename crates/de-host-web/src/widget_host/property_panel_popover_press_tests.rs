//! Web press tests for the property panel's #83 popovers (the Auto
//! layout alignment popover). Child of `property_panel_press_tests`.

use super::*;

#[test]
fn web_property_panel_alignment_popover_opens_and_sets_distribution() {
    let mut host = WidgetHost::new();
    seed(
        &mut host,
        r##"{ "version": "1.0.0", "children": [
              {"type":"group","id":"text_group","name":"Text Group",
               "layout":"vertical","gap":8,
               "children":[
                 {"type":"text","id":"label","name":"Label","content":"Hello"}
               ]}
        ]}"##,
    );
    host.editor_state
        .set_single_selection(NodeId::new("text_group"));

    let button = point_for_action(&host, |action| {
        matches!(action, PropertyPanelAction::ToggleLayoutAlignPopover)
    });
    assert!(host.apply_press(button.x, button.y, VIEWPORT_W, VIEWPORT_H));
    assert!(host.editor_state.editor_ui.layout_align_popover_open);

    let between = point_for_action(&host, |action| {
        matches!(
            action,
            PropertyPanelAction::SetLayoutJustify(LayoutJustifyValue::SpaceBetween)
        )
    });
    assert!(host.apply_press(between.x, between.y, VIEWPORT_W, VIEWPORT_H));
    assert_eq!(
        PropertyPanel::for_selection(&host.editor_state)
            .expect("panel")
            .snapshot
            .layout_justify,
        LayoutJustifyValue::SpaceBetween
    );
    // Picking a row keeps the popover open for further tweaks; a press
    // outside it closes it.
    assert!(host.editor_state.editor_ui.layout_align_popover_open);
    assert!(host.apply_press(4.0, VIEWPORT_H - 4.0, VIEWPORT_W, VIEWPORT_H));
    assert!(!host.editor_state.editor_ui.layout_align_popover_open);
}
