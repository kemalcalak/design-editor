//! #90: the paint / effect options the panel exposes on top of the
//! model — stroke alignment, inner shadow, flip, image Stretch, mesh
//! vertex colours and shader presets. Each test finds the control
//! through the shared action walker, proves `hit_test_action` reaches
//! it, then runs the shared dispatch.

use crate::layout_scene::SceneStrokeAlign;
use crate::widgets::property_panel::{EffectKind, PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for};
use crate::{Point2D, Rect};
use de_editor_core::fills::node_fills;
use de_editor_core::pen_node_ext::PenNodeExt;
use de_editor_core::{ColorTarget, EditorState, FillType, FlipAxis, NodeId, ShaderPreset};
use jian_ops_schema::style::{PenEffect, PenFill, StrokeAlign};

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, 280.0, 1400.0)
}

fn rect_state(extra: &str) -> EditorState {
    let mut state = state_from(&format!(
        r##"{{ "version": "1.0.0", "children": [
              {{"type":"rectangle","id":"r","name":"R","x":0,"y":0,"width":120,"height":80,
               "fill":[{{"type":"solid","color":"#3366ff"}}]{extra}}}
        ]}}"##
    ));
    state.set_single_selection(NodeId::new("r"));
    state
}

fn actions(state: &EditorState) -> (PropertyPanel, Vec<(PropertyPanelAction, Rect)>) {
    let panel = PropertyPanel::for_selection(state).expect("panel");
    let rects = sections::action_button_rects_with_fill_picker(
        panel_rect(),
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    );
    (panel, rects)
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

/// The walker emits `action` and a press at its centre resolves to it.
fn reachable(state: &EditorState, action: &PropertyPanelAction) -> Rect {
    let (panel, rects) = actions(state);
    let rect = rects
        .iter()
        .find(|(a, _)| a == action)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("walker emits {action:?}"));
    assert_eq!(
        panel.hit_test_action(panel_rect(), center(rect)).as_ref(),
        Some(action),
        "a press on {action:?} reaches it"
    );
    rect
}

fn dispatch(state: &mut EditorState, action: &PropertyPanelAction) {
    use crate::widgets::property_panel_dispatch as d;
    let mut image_adjustment_drag = None;
    let mut effect_radius_drag = None;
    let _ = d::apply_property_action(
        state,
        action,
        d::PropertyActionContext {
            now_ms: 0,
            resolved_sizing_fallback: None,
            image_adjustment_drag: &mut image_adjustment_drag,
            effect_radius_drag: &mut effect_radius_drag,
        },
    );
}

fn node(state: &EditorState) -> &jian_ops_schema::node::PenNode {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("r")).expect("node")
}

#[test]
fn stroke_alignment_row_writes_and_reads_back() {
    let mut state =
        rect_state(r##","stroke":{"thickness":2,"fill":[{"type":"solid","color":"#111111"}]}"##);
    let (panel, _) = actions(&state);
    assert_eq!(
        panel.snapshot.stroke.unwrap().align,
        SceneStrokeAlign::Center
    );
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        reachable(&state, &PropertyPanelAction::SetStrokeAlign(align.clone()));
    }

    let inside = PropertyPanelAction::SetStrokeAlign(StrokeAlign::Inside);
    assert!(sections_updates_document(&inside));
    dispatch(&mut state, &inside);
    assert_eq!(
        de_editor_core::fills::node_stroke_align(node(&state)),
        Some(StrokeAlign::Inside)
    );
    let (panel, _) = actions(&state);
    assert_eq!(
        panel.snapshot.stroke.unwrap().align,
        SceneStrokeAlign::Inside
    );
}

fn sections_updates_document(action: &PropertyPanelAction) -> bool {
    crate::widgets::property_panel_dispatch::updates_document(action)
}

#[test]
fn stroke_alignment_needs_an_existing_stroke() {
    let mut state = rect_state("");
    dispatch(
        &mut state,
        &PropertyPanelAction::SetStrokeAlign(StrokeAlign::Outside),
    );
    assert_eq!(de_editor_core::fills::node_stroke_align(node(&state)), None);
}

#[test]
fn effects_add_menu_offers_inner_shadow() {
    let mut state = rect_state("");
    state.editor_ui.toggle_effect_add_picker();
    let add = reachable_without_menu(&state, &PropertyPanelAction::ToggleEffectAddPicker);
    let menu = crate::widgets::property_panel_effects::effect_add_menu_rect(add);
    let rows = crate::widgets::property_panel_effects::effect_add_menu_row_rects(menu);
    let inner = PropertyPanelAction::AddEffect(EffectKind::InnerShadow);
    let (_, row) = rows.iter().find(|(a, _)| *a == inner).expect("inner row");
    let panel = PropertyPanel::for_selection(&state).unwrap();
    assert_eq!(
        panel.hit_test_action(panel_rect(), center(*row)),
        Some(inner.clone())
    );

    dispatch(&mut state, &inner);
    match de_editor_core::node_effects(node(&state)) {
        [PenEffect::Shadow(body)] => assert_eq!(body.inner, Some(true)),
        other => panic!("expected one inner shadow, got {other:?}"),
    }
    let panel = PropertyPanel::for_selection(&state).unwrap();
    assert_eq!(panel.snapshot.effects[0].kind, EffectKind::InnerShadow);
}

/// Like [`reachable`] without the hit test, for a rect an open menu
/// overlays.
fn reachable_without_menu(state: &EditorState, action: &PropertyPanelAction) -> Rect {
    actions(state)
        .1
        .into_iter()
        .find(|(a, _)| a == action)
        .map(|(_, r)| r)
        .expect("walker emits the action")
}

#[test]
fn position_header_flip_buttons_toggle_the_mirror_flags() {
    let mut state = rect_state("");
    let horizontal = PropertyPanelAction::FlipSelection(FlipAxis::Horizontal);
    let vertical = PropertyPanelAction::FlipSelection(FlipAxis::Vertical);
    let h = reachable(&state, &horizontal);
    let v = reachable(&state, &vertical);
    assert!(h.origin.x < v.origin.x, "horizontal sits left of vertical");

    dispatch(&mut state, &horizontal);
    assert_eq!(node(&state).base().flip_x, Some(true));
    let (panel, _) = actions(&state);
    assert!(panel.snapshot.flip_x && !panel.snapshot.flip_y);
    dispatch(&mut state, &horizontal);
    assert_eq!(node(&state).base().flip_x, None);
}

#[test]
fn image_fill_popover_offers_stretch_but_an_image_node_does_not() {
    let fill_state = rect_state("");
    let mut fill_state = fill_state;
    assert!(fill_state.set_selected_fill_type_at(0, FillType::Image));
    let panel = PropertyPanel::for_selection(&fill_state).unwrap();
    let modes = |panel: &PropertyPanel| -> Vec<de_editor_core::ImageFillMode> {
        crate::widgets::property_panel_image_fill::image_fill_popover_action_rects(
            panel_rect(),
            visible_for(panel),
            &panel.snapshot,
        )
        .into_iter()
        .filter_map(|(a, _)| match a {
            PropertyPanelAction::SetImageFillMode(mode) => Some(mode),
            _ => None,
        })
        .collect()
    };
    assert_eq!(modes(&panel), de_editor_core::ImageFillMode::ALL.to_vec());

    let stretch = PropertyPanelAction::SetImageFillMode(de_editor_core::ImageFillMode::Stretch);
    dispatch(&mut fill_state, &stretch);
    let panel = PropertyPanel::for_selection(&fill_state).unwrap();
    assert_eq!(
        panel.snapshot.image_fill.as_ref().unwrap().mode,
        de_editor_core::ImageFillMode::Stretch
    );

    let mut image_state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"image","id":"img","x":0,"y":0,"width":80,"height":60,"src":""}
        ]}"##,
    );
    image_state.set_single_selection(NodeId::new("img"));
    let panel = PropertyPanel::for_selection(&image_state).unwrap();
    assert_eq!(
        modes(&panel),
        de_editor_core::ImageFillMode::IMAGE_NODE.to_vec()
    );
    assert!(!image_state.set_selected_image_fill_mode(de_editor_core::ImageFillMode::Stretch));
}

#[test]
fn fill_picker_offers_mesh_and_shader() {
    use crate::widgets::property_panel_fill::fill_type_at;
    let offered: Vec<_> = (0..crate::widgets::property_panel_fill_picker::FILL_TYPE_COUNT)
        .filter_map(fill_type_at)
        .collect();
    assert!(offered.contains(&FillType::MeshGradient));
    assert!(offered.contains(&FillType::Shader));
}

#[test]
fn mesh_vertex_swatches_open_the_picker_and_write_the_vertex() {
    let mut state = rect_state("");
    assert!(state.set_selected_fill_type_at(0, FillType::MeshGradient));
    let (panel, rects) = actions(&state);
    assert_eq!(panel.snapshot.gradient_stops.len(), 4, "2×2 default mesh");
    for index in 0..4 {
        reachable(
            &state,
            &PropertyPanelAction::OpenColorPicker(ColorTarget::GradientStop(index)),
        );
    }
    // The vertex grid sits under the row and above the next section.
    let stroke_top = rects
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleStrokeModePopover))
        .map(|(_, r)| r.origin.y)
        .expect("stroke gear");
    let last_vertex = reachable(
        &state,
        &PropertyPanelAction::OpenColorPicker(ColorTarget::GradientStop(3)),
    );
    assert!(last_vertex.origin.y + last_vertex.size.y < stroke_top);

    assert!(state.set_selected_gradient_stop_hex(3, "#ff0000"));
    match node_fills(node(&state)).and_then(|f| f.first()) {
        Some(PenFill::MeshGradient(body)) => assert_eq!(body.stops[3].color, "#ff0000"),
        other => panic!("expected a mesh fill, got {other:?}"),
    }
}

#[test]
fn shader_preset_row_swaps_the_body() {
    let mut state = rect_state("");
    assert!(state.set_selected_fill_type_at(0, FillType::Shader));
    let (panel, _) = actions(&state);
    assert_eq!(
        panel.snapshot.shader_preset, None,
        "the default body is custom"
    );
    for preset in ShaderPreset::ALL {
        reachable(&state, &PropertyPanelAction::SetShaderPreset(preset));
    }

    dispatch(
        &mut state,
        &PropertyPanelAction::SetShaderPreset(ShaderPreset::Noise),
    );
    let (panel, _) = actions(&state);
    assert_eq!(panel.snapshot.shader_preset, Some(ShaderPreset::Noise));
}
