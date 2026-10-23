//! Size / layout geometry tests — fill + hug sizing keeping their
//! numeric inputs, the single shorthand padding input, and the
//! alignment controls living in the Auto layout header popover.
//!
//! Split out of `property_panel_tests.rs` to keep both files under
//! the design-editor 800-line cap.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_test_support::{state_from, visible_for, CountingBackend};
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use de_editor_core::NodeId;

#[test]
fn fill_width_keeps_both_numeric_inputs_visible_and_hittable() {
    use de_editor_core::PropertyFocus;
    let fill = {
        let mut s = state_from(
            r##"{ "version": "1.0.0", "children": [
                  {"type":"frame","id":"ff","name":"Frame",
                   "x":40,"y":40,"width":"fill_container","height":240,
                   "layout":"vertical","children":[]}
            ]}"##,
        );
        s.set_single_selection(NodeId::new("ff"));
        PropertyPanel::for_selection(&s).expect("fill-width frame panel")
    };
    assert!(fill.snapshot.size_fill_width, "width sizing should be fill");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let fill_rects = sections::editable_input_rects(
        rect,
        visible_for(&fill),
        &fill.snapshot.fills,
        &fill.snapshot.effects,
    );
    for focus in [PropertyFocus::SizeW, PropertyFocus::SizeH] {
        let target = fill_rects
            .iter()
            .find(|(candidate, _)| *candidate == focus)
            .map(|(_, rect)| *rect)
            .expect("fill sizing must keep the numeric input");
        let center = Point2D::new(
            target.origin.x + target.size.x / 2.0,
            target.origin.y + target.size.y / 2.0,
        );
        assert_eq!(fill.hit_test(rect, center), Some(focus));
    }
}

#[test]
fn both_dimensions_fill_keep_the_size_input_row() {
    use de_editor_core::PropertyFocus;
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let panel_for = |w: &str, h: &str| {
        let json = format!(
            r##"{{ "version": "1.0.0", "children": [
                  {{"type":"frame","id":"ff","name":"Frame",
                   "x":40,"y":40,"width":{w},"height":{h},
                   "layout":"vertical","children":[]}}
            ]}}"##
        );
        let mut s = state_from(&json);
        s.set_single_selection(NodeId::new("ff"));
        PropertyPanel::for_selection(&s).expect("frame panel")
    };
    let chk_y = |p: &PropertyPanel| {
        sections::action_button_rects_with_fill_picker(
            rect,
            visible_for(p),
            &p.snapshot.effects,
            &p.snapshot.fills,
            &p.snapshot.interactions,
            false,
            0,
            false,
            false,
            false,
            false,
            false,
        )
        .into_iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleSizeFillWidth))
        .map(|(_, r)| r.origin.y)
        .expect("fill-width checkbox rect")
    };
    let one = panel_for("\"fill_container\"", "240");
    let both = panel_for("\"fill_container\"", "\"fill_container\"");
    assert!(one.snapshot.size_fill_width && both.snapshot.size_fill_height);
    assert!(
        (chk_y(&one) - chk_y(&both)).abs() < 0.01,
        "checkbox rows must not jump when both axes use fill"
    );
    let both_inputs = sections::editable_input_rects(
        rect,
        visible_for(&both),
        &both.snapshot.fills,
        &both.snapshot.effects,
    );
    for focus in [PropertyFocus::SizeW, PropertyFocus::SizeH] {
        assert!(
            both_inputs.iter().any(|(candidate, _)| *candidate == focus),
            "both fill axes must still emit the numeric hit rect"
        );
    }
}

#[test]
fn fill_and_hug_inputs_paint_snapshot_size_and_numeric_commit_makes_fixed() {
    use de_editor_core::PropertyFocus;
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"frame","id":"ff","name":"Frame",
               "x":40,"y":40,"width":"fill_container","height":"fit_content",
               "layout":"vertical","children":[
                 {"type":"rectangle","id":"child","x":0,"y":0,
                  "width":180,"height":90}
               ]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("ff"));
    let panel = PropertyPanel::for_selection(&state).expect("fill/hug frame panel");
    assert_eq!((panel.snapshot.width, panel.snapshot.height), (180, 90));
    assert!(panel.snapshot.size_fill_width && panel.snapshot.size_hug_height);

    let mut backend = CountingBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    panel.paint(
        &mut cx,
        Rect {
            origin: Point2D::ZERO,
            size: Point2D::new(280.0, 1200.0),
        },
    );
    assert!(backend.texts.iter().any(|text| text == "180"));
    assert!(backend.texts.iter().any(|text| text == "90"));
    assert!(state.commit_property_edit(PropertyFocus::SizeW, 220.0));
    assert!(state.commit_property_edit(PropertyFocus::SizeH, 130.0));
    let fixed = PropertyPanel::for_selection(&state).expect("fixed frame panel");
    assert_eq!((fixed.snapshot.width, fixed.snapshot.height), (220, 130));
    assert!(!fixed.snapshot.size_fill_width && !fixed.snapshot.size_hug_height);
}

#[test]
fn padding_is_one_shorthand_input_whatever_the_values() {
    use de_editor_core::PropertyFocus;
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let padding = |padding: &str| {
        let json = format!(
            r##"{{ "version": "1.0.0", "children": [
                  {{"type":"frame","id":"f","name":"F","x":0,"y":0,
                   "width":300,"height":200,"layout":"vertical",
                   "padding":{padding},"children":[]}}
            ]}}"##
        );
        let mut s = state_from(&json);
        s.set_single_selection(NodeId::new("f"));
        let panel = PropertyPanel::for_selection(&s).expect("frame panel");
        let inputs = sections::editable_input_rects(
            rect,
            visible_for(&panel),
            &panel.snapshot.fills,
            &panel.snapshot.effects,
        )
        .into_iter()
        .filter(|(f, _)| *f == PropertyFocus::Padding)
        .count();
        (inputs, panel.snapshot.layout_padding.shorthand())
    };
    assert_eq!(padding("12"), (1, "12".to_string()));
    assert_eq!(padding("[10, 20]"), (1, "10 20".to_string()));
    assert_eq!(padding("[8, 24, 32, 24]"), (1, "8 24 32".to_string()));
    assert_eq!(padding("[8, 24, 32, 4]"), (1, "8 24 32 4".to_string()));
}

#[test]
fn alignment_controls_live_in_the_header_popover() {
    use crate::widgets::property_panel::LayoutJustifyValue;
    use crate::widgets::property_panel_flex::align_popover_rect_from_button;
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"frame","id":"f","name":"Frame",
               "x":40,"y":40,"width":360,"height":240,
               "layout":"horizontal","gap":0,
               "children":[]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("f"));
    let panel = PropertyPanel::for_selection(&state).expect("frame panel");
    let rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(280.0, 1200.0),
    };
    let visible = visible_for(&panel);
    let actions = |open: bool| {
        sections::action_button_rects_with_fill_picker(
            rect,
            visible,
            &panel.snapshot.effects,
            &panel.snapshot.fills,
            &panel.snapshot.interactions,
            false,
            0,
            false,
            false,
            false,
            false,
            open,
        )
    };
    let is_alignment = |action: &PropertyPanelAction| {
        matches!(
            action,
            PropertyPanelAction::SetLayoutAlignment { .. }
                | PropertyPanelAction::SetLayoutAlign(_)
                | PropertyPanelAction::SetLayoutJustify(_)
        )
    };
    let closed = actions(false);
    assert!(
        !closed.iter().any(|(a, _)| is_alignment(a)),
        "a closed popover leaves no alignment targets in the section"
    );
    let button = closed
        .iter()
        .find_map(|(a, r)| matches!(a, PropertyPanelAction::ToggleLayoutAlignPopover).then_some(*r))
        .expect("header alignment button");
    let popover = align_popover_rect_from_button(button, rect.size.x);
    let open = actions(true);
    let targets: Vec<Rect> = open
        .iter()
        .filter(|(a, _)| is_alignment(a))
        .map(|(_, r)| *r)
        .collect();
    assert_eq!(targets.len(), 12, "3x3 grid + 3 distribution rows");
    for r in targets {
        assert!(
            r.origin.x >= popover.origin.x
                && r.origin.y >= popover.origin.y
                && r.origin.x + r.size.x <= popover.origin.x + popover.size.x
                && r.origin.y + r.size.y <= popover.origin.y + popover.size.y,
            "alignment target {r:?} must sit inside the popover {popover:?}"
        );
    }
    assert!(open.iter().any(|(a, _)| matches!(
        a,
        PropertyPanelAction::SetLayoutJustify(LayoutJustifyValue::SpaceAround)
    )));
}

#[test]
fn open_alignment_popover_paints_its_distribution_rows() {
    let mut state = state_from(
        r##"{ "version": "1.0.0", "children": [
              {"type":"frame","id":"f","name":"Frame",
               "x":40,"y":40,"width":360,"height":240,
               "layout":"vertical","gap":8,"children":[]}
        ]}"##,
    );
    state.set_single_selection(NodeId::new("f"));
    state.editor_ui.locale = de_editor_core::Locale::EnUs;
    let paints_rows = |state: &de_editor_core::EditorState| {
        let panel = PropertyPanel::for_selection(state).expect("frame panel");
        let mut backend = CountingBackend::default();
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(
            &mut cx,
            Rect {
                origin: Point2D::ZERO,
                size: Point2D::new(280.0, 1200.0),
            },
        );
        backend.texts.iter().any(|text| text == "Space Between")
    };
    assert!(!paints_rows(&state), "closed popover paints nothing");
    state.editor_ui.layout_align_popover_open = true;
    assert!(paints_rows(&state), "open popover paints its rows");
}
