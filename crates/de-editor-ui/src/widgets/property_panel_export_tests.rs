use super::property_panel::{PropertyPanel, PropertyPanelAction};
use super::property_panel_sections as sections;
use super::property_panel_test_support::visible_for;
use crate::Rect;
use de_editor_core::{EditorState, ExportFormat, NodeId};

fn export_state(format: ExportFormat) -> EditorState {
    let mut state = EditorState::sample();
    state.set_single_selection(NodeId::new("n10"));
    state.editor_ui.export_section_open = true;
    state.editor_ui.export_format = format;
    state
}

fn export_rects(
    state: &EditorState,
    scale_open: bool,
    format_open: bool,
) -> Vec<(PropertyPanelAction, Rect)> {
    let panel = PropertyPanel::for_selection(state).expect("frame panel");
    sections::action_button_rects_with_fill_picker(
        Rect::xywh(0.0, 0.0, 280.0, 1600.0),
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        scale_open,
        format_open,
        false,
    )
}

fn rect_of(rects: &[(PropertyPanelAction, Rect)], action: PropertyPanelAction) -> Option<Rect> {
    rects.iter().find(|(a, _)| *a == action).map(|(_, r)| *r)
}

#[test]
fn export_format_picker_lists_every_dialog_format() {
    let state = export_state(ExportFormat::Png);
    let formats: Vec<_> = export_rects(&state, false, true)
        .into_iter()
        .filter_map(|(action, _)| match action {
            PropertyPanelAction::SetExportFormat(format) => Some(format),
            _ => None,
        })
        .collect();

    assert_eq!(formats, ExportFormat::ALL);
}

#[test]
fn raster_format_keeps_scale_dropdown_beside_format() {
    let rects = export_rects(&export_state(ExportFormat::Png), false, false);
    let scale = rect_of(&rects, PropertyPanelAction::ToggleExportScalePicker)
        .expect("PNG shows the scale dropdown");
    let format = rect_of(&rects, PropertyPanelAction::ToggleExportFormatPicker).expect("format");
    let button = rect_of(&rects, PropertyPanelAction::ExportImageNow).expect("export button");

    assert_eq!(scale.origin.y, format.origin.y);
    assert!(scale.origin.x + scale.size.x < format.origin.x);
    assert!(format.size.x < button.size.x);
}

#[test]
fn non_raster_format_hides_scale_and_widens_format() {
    for format in [
        ExportFormat::Svg,
        ExportFormat::Pdf,
        ExportFormat::Html,
        ExportFormat::HtmlTailwind,
    ] {
        let rects = export_rects(&export_state(format), true, false);
        assert_eq!(
            rect_of(&rects, PropertyPanelAction::ToggleExportScalePicker),
            None,
            "{format:?} shows no scale dropdown"
        );
        assert!(
            !rects
                .iter()
                .any(|(a, _)| matches!(a, PropertyPanelAction::SetExportScale(_))),
            "{format:?} emits no scale picker rows"
        );
        let format_rect = rect_of(&rects, PropertyPanelAction::ToggleExportFormatPicker)
            .expect("format dropdown");
        let button = rect_of(&rects, PropertyPanelAction::ExportImageNow).expect("export button");
        assert_eq!(format_rect.origin.x, button.origin.x, "{format:?}");
        assert_eq!(format_rect.size.x, button.size.x, "{format:?}");
    }
}

#[test]
fn format_pick_closes_scale_picker_only_for_non_raster() {
    use crate::widgets::property_panel_dispatch as dispatch;
    for (format, scale_open) in [(ExportFormat::Html, false), (ExportFormat::Jpeg, true)] {
        let mut state = export_state(ExportFormat::Png);
        state.editor_ui.export_scale_picker_open = true;
        let mut image_adjustment_drag = None;
        let mut effect_radius_drag = None;
        let _ = dispatch::apply_property_action(
            &mut state,
            &PropertyPanelAction::SetExportFormat(format),
            dispatch::PropertyActionContext {
                now_ms: 0,
                resolved_sizing_fallback: None,
                image_adjustment_drag: &mut image_adjustment_drag,
                effect_radius_drag: &mut effect_radius_drag,
            },
        );
        assert_eq!(state.editor_ui.export_format, format);
        assert_eq!(
            state.editor_ui.export_scale_picker_open, scale_open,
            "{format:?}"
        );
    }
}
