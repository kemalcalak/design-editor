//! #379 through the Auto layout section: a card grid and a wrapping tag
//! list are built with one frame each, by pressing the panel's own
//! controls and committing its fields.

use super::property_panel::{PropertyPanel, PropertyPanelAction};
use super::property_panel_commit::{commit_property_focus, property_focus_initial};
use super::property_panel_layout_ops as layout_ops;
use super::property_panel_test_support::{state_from, CountingBackend};
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use de_editor_core::{EditorState, FlexLayout, NodeId, PropertyFocus};
use jian_ops_schema::node::{GridTemplate, LayoutMode, PenNode};

const DOC: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"f","name":"Cards","width":340,"height":200,"layout":"vertical","children":[
    {"type":"rectangle","id":"a","width":80,"height":40},
    {"type":"rectangle","id":"b","width":80,"height":40},
    {"type":"rectangle","id":"c","width":80,"height":40}]}]}"##;

const PANEL: Rect = Rect {
    origin: Point2D { x: 0.0, y: 0.0 },
    size: Point2D {
        x: 280.0,
        y: 1600.0,
    },
};

fn panel(state: &mut EditorState) -> PropertyPanel {
    state.set_single_selection(NodeId::new("f"));
    PropertyPanel::for_selection(state).expect("panel builds")
}

fn points() -> impl Iterator<Item = Point2D> {
    (0..400)
        .flat_map(|row| (0..70).map(move |col| Point2D::new(col as f32 * 4.0, row as f32 * 4.0)))
}

fn action_point(panel: &PropertyPanel, action: &PropertyPanelAction) -> Option<Point2D> {
    points().find(|p| panel.hit_test_action(PANEL, *p).as_ref() == Some(action))
}

fn has_focus(panel: &PropertyPanel, focus: PropertyFocus) -> bool {
    points().any(|p| panel.hit_test(PANEL, p) == Some(focus))
}

fn container(state: &EditorState) -> jian_ops_schema::node::ContainerProps {
    match state.selected_node().unwrap() {
        PenNode::Frame(f) => f.container.clone(),
        other => panic!("{other:?}"),
    }
}

fn commit(state: &mut EditorState, focus: PropertyFocus, text: &str) {
    state.ui.property_focus = Some(focus);
    state.ui.property_input.set_text(text);
    assert!(commit_property_focus(state), "{focus:?}");
}

fn paint(panel: &PropertyPanel) -> Vec<String> {
    let mut backend = CountingBackend::default();
    panel.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        PANEL,
    );
    backend.texts
}

#[test]
fn a_card_grid_from_the_panel() {
    let mut state = state_from(DOC);
    let p = panel(&mut state);
    assert!(action_point(&p, &PropertyPanelAction::SetFlexLayout(FlexLayout::Grid)).is_some());
    layout_ops::set_selected_layout_mode(&mut state, FlexLayout::Grid);
    assert_eq!(container(&state).layout, Some(LayoutMode::Grid));
    assert_eq!(container(&state).columns, Some(GridTemplate::Count(2)));

    let p = panel(&mut state);
    assert_eq!(p.snapshot.flex_layout, FlexLayout::Grid);
    for focus in [
        PropertyFocus::GridColumns,
        PropertyFocus::GridRows,
        PropertyFocus::LayoutRowGap,
    ] {
        assert!(has_focus(&p, focus), "{focus:?} has no field");
    }
    assert_eq!(property_focus_initial(PropertyFocus::GridColumns, &p), "2");
    commit(&mut state, PropertyFocus::GridColumns, "1fr 1fr 1fr");
    commit(&mut state, PropertyFocus::LayoutRowGap, "12");
    let c = container(&state);
    assert_eq!(c.columns.as_ref().map(GridTemplate::len), Some(3));
    assert_eq!(
        c.row_gap,
        Some(jian_ops_schema::node::NumberOrExpression::Number(12.0))
    );
    // A bad list writes nothing.
    commit(&mut state, PropertyFocus::GridColumns, "1fr nope");
    assert_eq!(
        container(&state).columns.as_ref().map(GridTemplate::len),
        Some(3)
    );

    let p = panel(&mut state);
    let texts = paint(&p);
    assert!(texts.iter().any(|t| t == "1fr 1fr 1fr"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "Columns"), "{texts:?}");
}

#[test]
fn a_wrapping_tag_list_from_the_panel() {
    let mut state = state_from(DOC);
    state.set_single_selection(NodeId::new("f"));
    layout_ops::set_selected_layout_mode(&mut state, FlexLayout::Horizontal);
    let p = panel(&mut state);
    assert!(has_focus(&p, PropertyFocus::LayoutRowGap));
    assert!(action_point(&p, &PropertyPanelAction::ToggleLayoutWrap).is_some());
    layout_ops::toggle_selected_wrap(&mut state);
    assert_eq!(container(&state).wrap, Some(true));
    let p = panel(&mut state);
    assert!(p.snapshot.layout_grid.wrap);
    assert!(paint(&p).iter().any(|t| t == "Wrap"));
    layout_ops::toggle_selected_wrap(&mut state);
    assert_eq!(container(&state).wrap, None);
    // Vertical has neither control.
    layout_ops::set_selected_layout_mode(&mut state, FlexLayout::Vertical);
    let p = panel(&mut state);
    assert!(action_point(&p, &PropertyPanelAction::ToggleLayoutWrap).is_none());
    assert!(!has_focus(&p, PropertyFocus::LayoutRowGap));
}
