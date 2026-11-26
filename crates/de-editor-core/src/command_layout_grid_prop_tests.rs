use crate::command::{EditorCommand, LayoutPropValue};
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{rect, state_with};
use crate::walkers::find_node;
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::grid::{GridPlacement, GridTemplate, GridTrack, SelfAlign, TrackSize};
use jian_ops_schema::node::{NumberOrExpression, PenNode};

fn set(s: &mut crate::EditorState, property: &str, value: LayoutPropValue) -> bool {
    s.apply(EditorCommand::SetNodeLayoutProp {
        node_id: NodeId::new("n1"),
        property: property.into(),
        value,
    })
}

fn container(s: &crate::EditorState) -> jian_ops_schema::node::ContainerProps {
    match find_node(s.active_children(), &NodeId::new("n1")).unwrap() {
        PenNode::Rectangle(r) => r.container.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn switching_to_grid_seeds_two_columns_once() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    assert!(set(
        &mut s,
        "layout",
        LayoutPropValue::Keyword("grid".into())
    ));
    let c = container(&s);
    assert_eq!(c.layout, Some(LayoutMode::Grid));
    assert_eq!(c.columns, Some(GridTemplate::Count(2)));
    // Authored tracks survive a later switch back to grid.
    assert!(set(
        &mut s,
        "columns",
        LayoutPropValue::Keyword("120 1fr".into())
    ));
    assert!(set(
        &mut s,
        "layout",
        LayoutPropValue::Keyword("vertical".into())
    ));
    assert!(set(
        &mut s,
        "layout",
        LayoutPropValue::Keyword("grid".into())
    ));
    assert_eq!(
        container(&s).columns,
        Some(GridTemplate::Tracks(vec![
            GridTrack::Size(TrackSize::Px(120.0)),
            GridTrack::FR1
        ]))
    );
}

#[test]
fn wrap_gaps_and_tracks_write_and_validate() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    assert!(set(&mut s, "wrap", LayoutPropValue::Bool(true)));
    assert!(set(&mut s, "rowGap", LayoutPropValue::Number(12.0)));
    assert!(set(&mut s, "columnGap", LayoutPropValue::Number(-3.0)));
    assert!(set(&mut s, "columns", LayoutPropValue::Number(3.0)));
    assert!(set(
        &mut s,
        "rows",
        LayoutPropValue::Keyword("auto 40".into())
    ));
    let c = container(&s);
    assert_eq!(c.wrap, Some(true));
    assert_eq!(c.row_gap, Some(NumberOrExpression::Number(12.0)));
    assert_eq!(c.column_gap, Some(NumberOrExpression::Number(0.0)));
    assert_eq!(c.columns, Some(GridTemplate::Count(3)));
    assert_eq!(c.rows.as_ref().map(GridTemplate::len), Some(2));

    // Bad values write nothing.
    assert!(!set(
        &mut s,
        "columns",
        LayoutPropValue::Keyword("1fr nope".into())
    ));
    assert!(!set(&mut s, "columns", LayoutPropValue::Number(0.0)));
    assert!(!set(&mut s, "wrap", LayoutPropValue::Number(1.0)));
    assert_eq!(container(&s).columns, Some(GridTemplate::Count(3)));

    // Off clears the flag; an empty rows list clears the rows.
    assert!(set(&mut s, "wrap", LayoutPropValue::Bool(false)));
    assert!(set(&mut s, "rows", LayoutPropValue::Keyword(String::new())));
    let c = container(&s);
    assert_eq!(c.wrap, None);
    assert_eq!(c.rows, None);
}

#[test]
fn child_placement_and_alignment() {
    let mut s = state_with(vec![rect("n1", "r", 0.0, 0.0, 10.0, 10.0)]);
    assert!(set(
        &mut s,
        "gridColumn",
        LayoutPropValue::Keyword("1 / span 2".into())
    ));
    assert!(set(&mut s, "gridRow", LayoutPropValue::Number(3.0)));
    assert!(set(
        &mut s,
        "justifySelf",
        LayoutPropValue::Keyword("end".into())
    ));
    assert!(set(
        &mut s,
        "alignSelf",
        LayoutPropValue::Keyword("stretch".into())
    ));
    let node = find_node(s.active_children(), &NodeId::new("n1")).unwrap();
    let base = node.base();
    assert_eq!(
        base.grid_column,
        Some(GridPlacement {
            start: Some(1),
            span: 2
        })
    );
    assert_eq!(base.grid_row.and_then(|p| p.start), Some(3));
    assert_eq!(base.justify_self, Some(SelfAlign::End));
    assert_eq!(base.align_self, Some(SelfAlign::Stretch));
    assert!(!set(
        &mut s,
        "gridColumn",
        LayoutPropValue::Keyword("0".into())
    ));
    assert!(set(
        &mut s,
        "gridColumn",
        LayoutPropValue::Keyword("auto".into())
    ));
    let node = find_node(s.active_children(), &NodeId::new("n1")).unwrap();
    assert_eq!(node.base().grid_column, None);
}
