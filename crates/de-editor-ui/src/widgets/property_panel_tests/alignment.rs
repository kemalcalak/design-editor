//! Paint / hit-test alignment across the whole section stack (#83):
//! every y-walk (paint, the action walker, the input walker, the text
//! section's own anchor walk) must land the same sections at the same
//! y, and a frame like the editor design's fits a 1440×900 window.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_action::CodegenAction;
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::widgets::test_capture_backend::CaptureBackend;
use crate::widgets::{PaintCx, Widget};
use crate::Rect;
use de_editor_core::codegen::Framework;
use de_editor_core::{EditorState, NodeId};

const PANEL_W: f32 = 272.0;

fn tall_rect() -> Rect {
    Rect::xywh(0.0, 0.0, PANEL_W, 3000.0)
}

fn select(json: &str, id: &str) -> EditorState {
    let mut state = state_from(json);
    state.set_single_selection(NodeId::new(id));
    state
}

/// The closing Code chips are the last thing every walk reaches, so the
/// painted "React" label sitting inside the walker's React chip proves
/// paint and the action walker agree on every section above it.
fn assert_code_chips_aligned(state: &EditorState, case: &str) {
    let panel = PropertyPanel::for_selection(state).expect(case);
    let rect = tall_rect();
    let chip = sections::action_button_rects(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
    )
    .into_iter()
    .find_map(|(action, r)| {
        matches!(
            action,
            PropertyPanelAction::Codegen(CodegenAction::OpenInCodeView(Framework::React))
        )
        .then_some(r)
    })
    .unwrap_or_else(|| panic!("{case}: React chip rect"));
    let mut backend = CaptureBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        rect,
    );
    let (_, origin) = backend
        .texts
        .iter()
        .find(|(text, _)| text == "React")
        .unwrap_or_else(|| panic!("{case}: React label painted"));
    assert!(
        origin.y > chip.origin.y && origin.y < chip.origin.y + chip.size.y,
        "{case}: painted chip baseline {} outside walker chip {:?}",
        origin.y,
        chip
    );
}

#[test]
fn paint_and_action_walker_agree_through_every_section() {
    let frame = r##"{ "version": "1.0.0", "children": [
          {"type":"frame","id":"f","name":"Pricing","x":0,"y":0,"width":1440,"height":900,
           "layout":"vertical","gap":24,"padding":[96,120],"cornerRadius":[4,8,12,16],
           "fill":[{"type":"solid","color":"#0F1A14"},
                   {"type":"linear_gradient","angle":90,"stops":[
                     {"offset":0,"color":"#3B82F6"},{"offset":1,"color":"#8B5CF6"}]}],
           "effects":[{"type":"background_blur","radius":24}],
           "events":{"onTap":[{"pop":null}]},
           "children":[]}
    ]}"##;
    let mut state = select(frame, "f");
    assert_code_chips_aligned(&state, "frame");

    state.editor_ui.corner_expand_open = true;
    state.editor_ui.export_section_open = true;
    state.editor_ui.layout_align_popover_open = true;
    state.editor_ui.fill_menu_open = Some(1);
    assert_code_chips_aligned(&state, "frame, corner grid + export open, popovers up");

    let text = r##"{ "version": "1.0.0", "children": [
          {"type":"text","id":"t","name":"Title","content":"Hi","fontSize":24,
           "fill":[{"type":"solid","color":"#FFFFFF"}]}
    ]}"##;
    assert_code_chips_aligned(&select(text, "t"), "text");

    let rect = r##"{ "version": "1.0.0", "children": [
          {"type":"rectangle","id":"r","name":"R","x":0,"y":0,"width":80,"height":40,
           "fill":[{"type":"solid","color":"#112233"},{"type":"solid","color":"#445566"}],
           "stroke":{"thickness":1,"fill":[{"type":"solid","color":"#374151"}]}}
    ]}"##;
    assert_code_chips_aligned(&select(rect, "r"), "rectangle with stroke");
}

#[test]
fn text_anchor_walk_matches_the_action_walker() {
    let text = r##"{ "version": "1.0.0", "children": [
          {"type":"text","id":"t","name":"Title","content":"Hi","fontSize":24}
    ]}"##;
    let state = select(text, "t");
    let panel = PropertyPanel::for_selection(&state).expect("text panel");
    let rect = tall_rect();
    let visible = visible_for(&panel);
    let from_walker = sections::action_button_rects(
        rect,
        visible,
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
    )
    .into_iter()
    .find_map(|(a, r)| matches!(a, PropertyPanelAction::ToggleFontWeightPicker).then_some(r))
    .expect("weight dropdown from the action walker");
    let text_top = crate::widgets::property_panel_text::sections_top_before_text(rect, visible);
    let from_anchor_walk = crate::widgets::property_panel_text::text_action_rects(
        rect.origin.x,
        text_top,
        rect.size.x - crate::widgets::property_panel_inputs::PAD_X * 2.0,
    )
    .into_iter()
    .find_map(|(a, r)| matches!(a, PropertyPanelAction::ToggleFontWeightPicker).then_some(r))
    .expect("weight dropdown from the text anchor walk");
    assert_eq!(from_walker, from_anchor_walk);
}

#[test]
fn a_design_frame_fits_position_through_code_at_1440x900() {
    // The editor frame's selection: auto layout, one fill, one effect,
    // no interactions, Export collapsed (the default).
    let frame = r##"{ "version": "1.0.0", "children": [
          {"type":"frame","id":"f","name":"Pricing","x":0,"y":0,"width":1440,"height":900,
           "layout":"vertical","gap":24,"padding":[96,120],
           "fill":[{"type":"solid","color":"#0F1A14"}],
           "effects":[{"type":"background_blur","radius":24}],
           "children":[]}
    ]}"##;
    let state = select(frame, "f");
    let panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let rect = Rect::xywh(0.0, 0.0, PANEL_W, 900.0);
    let content = sections::property_panel_content_height(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
    );
    assert!(
        content <= rect.size.y,
        "Position through Code must fit a 900 px panel, content is {content} px"
    );
}

#[test]
fn painted_input_values_sit_inside_the_input_walker_rects() {
    use de_editor_core::PropertyFocus;
    let frame = r##"{ "version": "1.0.0", "children": [
          {"type":"frame","id":"f","name":"Pricing","x":0,"y":0,"width":1440,"height":900,
           "layout":"vertical","gap":24,"padding":[96,120],
           "fill":[{"type":"solid","color":"#0F1A14"},{"type":"solid","color":"#445566"}],
           "children":[]}
    ]}"##;
    let state = select(frame, "f");
    let panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let rect = tall_rect();
    let inputs = sections::editable_input_rects(
        rect,
        visible_for(&panel),
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    );
    let mut backend = CaptureBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        rect,
    );
    for (focus, value) in [
        (PropertyFocus::LayoutGap, "24"),
        (PropertyFocus::Padding, "96 120"),
        (PropertyFocus::FillHex(0), "#0F1A14"),
        (PropertyFocus::FillHex(1), "#445566"),
        (PropertyFocus::FillOpacity(1), "100"),
    ] {
        let field = inputs
            .iter()
            .find_map(|(f, r)| (*f == focus).then_some(*r))
            .unwrap_or_else(|| panic!("{focus:?}: input rect"));
        assert!(
            backend.texts.iter().any(|(text, origin)| {
                text == value
                    && origin.y > field.origin.y
                    && origin.y < field.origin.y + field.size.y
            }),
            "{focus:?}: `{value}` must paint inside its input rect {field:?}"
        );
    }
}
