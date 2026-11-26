//! `SetNodeLayoutProp` arms for wrap and grid auto layout (#379).
//!
//! Container: `wrap` (bool), `rowGap` / `columnGap` (number),
//! `columns` / `rows` (a count, or a CSS track list such as
//! `"1fr 1fr 120"` / `"repeat(3, 1fr)"`; an empty keyword clears rows).
//! Child of a grid: `gridColumn` / `gridRow` (`2`, `"span 2"`,
//! `"1 / span 3"`, `"auto"` clears) and `justifySelf` / `alignSelf`
//! (`start` / `center` / `end` / `stretch`, `auto` clears).
//!
//! Validate-then-mutate like the other arms: an unparsable value writes
//! nothing and returns `false`.

use crate::command::LayoutPropValue;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::walkers::find_node_mut;
use jian_ops_schema::node::base::NumberOrExpression;
use jian_ops_schema::node::container::ContainerProps;
use jian_ops_schema::node::grid::{GridPlacement, GridTemplate, SelfAlign};
use jian_ops_schema::node::{PenNode, ScriptNode};

/// Columns a container gets when it switches to grid without any.
pub const DEFAULT_GRID_COLUMNS: u32 = 2;

/// One validated grid / wrap write.
#[derive(Debug, Clone, PartialEq)]
enum GridWrite {
    Wrap(bool),
    RowGap(f64),
    ColumnGap(f64),
    Columns(Option<GridTemplate>),
    Rows(Option<GridTemplate>),
    GridColumn(Option<GridPlacement>),
    GridRow(Option<GridPlacement>),
    JustifySelf(Option<SelfAlign>),
    AlignSelf(Option<SelfAlign>),
}

fn template(value: &LayoutPropValue) -> Option<Option<GridTemplate>> {
    match value {
        LayoutPropValue::Number(n) if n.is_finite() && *n >= 1.0 && n.fract() == 0.0 => Some(Some(
            GridTemplate::Count((*n as u32).min(GridTemplate::MAX_TRACKS)),
        )),
        LayoutPropValue::Keyword(s) if s.trim().is_empty() => Some(None),
        LayoutPropValue::Keyword(s) => GridTemplate::parse(s).filter(|t| !t.is_empty()).map(Some),
        _ => None,
    }
}

fn placement(value: &LayoutPropValue) -> Option<Option<GridPlacement>> {
    match value {
        LayoutPropValue::Number(n) if n.fract() == 0.0 && (1.0..=65535.0).contains(n) => {
            Some(Some(GridPlacement {
                start: Some(*n as u16),
                span: 1,
            }))
        }
        LayoutPropValue::Keyword(s) if matches!(s.trim(), "" | "auto") => Some(None),
        LayoutPropValue::Keyword(s) => GridPlacement::parse(s).map(Some),
        _ => None,
    }
}

fn self_align(value: &LayoutPropValue) -> Option<Option<SelfAlign>> {
    let LayoutPropValue::Keyword(s) = value else {
        return None;
    };
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Some(None),
        "start" | "flex-start" | "left" | "top" => Some(Some(SelfAlign::Start)),
        "center" | "middle" => Some(Some(SelfAlign::Center)),
        "end" | "flex-end" | "right" | "bottom" => Some(Some(SelfAlign::End)),
        "stretch" => Some(Some(SelfAlign::Stretch)),
        _ => None,
    }
}

fn gap(value: &LayoutPropValue) -> Option<f64> {
    match value {
        LayoutPropValue::Number(n) if n.is_finite() => Some(n.max(0.0)),
        _ => None,
    }
}

/// `Ok(Some(write))` for a property this module owns with a valid value,
/// `Err(())` for one of its properties with a bad value, `Ok(None)` for
/// any other property.
fn parse(property: &str, value: &LayoutPropValue) -> Result<Option<GridWrite>, ()> {
    let write = match property {
        "wrap" => match value {
            LayoutPropValue::Bool(b) => GridWrite::Wrap(*b),
            _ => return Err(()),
        },
        "rowGap" => GridWrite::RowGap(gap(value).ok_or(())?),
        "columnGap" => GridWrite::ColumnGap(gap(value).ok_or(())?),
        "columns" => GridWrite::Columns(template(value).ok_or(())?),
        "rows" => GridWrite::Rows(template(value).ok_or(())?),
        "gridColumn" => GridWrite::GridColumn(placement(value).ok_or(())?),
        "gridRow" => GridWrite::GridRow(placement(value).ok_or(())?),
        "justifySelf" => GridWrite::JustifySelf(self_align(value).ok_or(())?),
        "alignSelf" => GridWrite::AlignSelf(self_align(value).ok_or(())?),
        _ => return Ok(None),
    };
    Ok(Some(write))
}

fn container_mut(node: &mut PenNode) -> Option<&mut ContainerProps> {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => Some(&mut n.container),
        PenNode::Group(n) => Some(&mut n.container),
        PenNode::Rectangle(n) => Some(&mut n.container),
        _ => None,
    }
}

fn apply(node: &mut PenNode, write: GridWrite) -> bool {
    let number = |v: f64| Some(NumberOrExpression::Number(v));
    match write {
        GridWrite::GridColumn(p) => node.base_mut().grid_column = p,
        GridWrite::GridRow(p) => node.base_mut().grid_row = p,
        GridWrite::JustifySelf(a) => node.base_mut().justify_self = a,
        GridWrite::AlignSelf(a) => node.base_mut().align_self = a,
        container_write => {
            let Some(c) = container_mut(node) else {
                return false;
            };
            match container_write {
                GridWrite::Wrap(b) => c.wrap = b.then_some(true),
                GridWrite::RowGap(v) => c.row_gap = number(v),
                GridWrite::ColumnGap(v) => c.column_gap = number(v),
                GridWrite::Columns(t) => c.columns = t,
                GridWrite::Rows(t) => c.rows = t,
                _ => unreachable!("child writes handled above"),
            }
        }
    }
    true
}

impl EditorState {
    /// Grid / wrap arm of `SetNodeLayoutProp`. `None` when `property`
    /// is not one of this module's; otherwise whether it was written.
    pub(crate) fn cmd_set_grid_layout_prop(
        &mut self,
        node_id: &NodeId,
        property: &str,
        value: &LayoutPropValue,
    ) -> Option<bool> {
        let write = match parse(property, value) {
            Ok(Some(write)) => write,
            Ok(None) => return None,
            Err(()) => return Some(false),
        };
        if !node_id.is_real() {
            return Some(false);
        }
        let Some(node) = find_node_mut(self.active_children_mut(), node_id) else {
            return Some(false);
        };
        let changed = apply(node, write);
        if changed {
            self.editor_ui.preserve_authored_geometry = false;
        }
        Some(changed)
    }

    /// Commit the property panel's Columns / Rows draft onto the
    /// selection. An empty draft clears rows (columns keep their value).
    pub fn commit_grid_tracks(&mut self, rows: bool, draft: &str) -> bool {
        let id = self.selection.anchor.clone();
        let property = if rows { "rows" } else { "columns" };
        let draft = draft.trim();
        if !rows && draft.is_empty() {
            return false;
        }
        self.cmd_set_node_layout_prop(&id, property, &LayoutPropValue::Keyword(draft.into()))
    }
}

/// A container switching to grid gets [`DEFAULT_GRID_COLUMNS`] equal
/// columns when it has none, so its children spread out at once.
pub(crate) fn seed_grid_columns(c: &mut ContainerProps) {
    if c.columns.as_ref().is_none_or(GridTemplate::is_empty) {
        c.columns = Some(GridTemplate::Count(DEFAULT_GRID_COLUMNS));
    }
}

#[cfg(test)]
#[path = "command_layout_grid_prop_tests.rs"]
mod tests;
