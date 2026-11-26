//! Grid and wrap containers for the absolutely positioned targets (#379).
//!
//! The React / Vue / Svelte / React Native / Flutter / SwiftUI / Compose
//! generators place every node at its authored offset, which says nothing
//! about a grid's cells or a wrapping row: the layout decides those. So a
//! `layout: grid` container, or a horizontal one with `wrap: true`, is
//! emitted with the platform's own layout and its children flow inside it:
//!
//! | target          | grid                         | wrap                            |
//! | --------------- | ---------------------------- | ------------------------------- |
//! | HTML / JSX      | `display: grid`              | `display: flex; flex-wrap: wrap`|
//! | React Native    | `flexWrap: 'wrap'` row       | `flexWrap: 'wrap'` row          |
//! | Flutter         | `GridView.count`             | `Wrap`                          |
//! | SwiftUI         | `LazyVGrid` (per-track items)| `LazyVGrid` with `.adaptive`    |
//! | Compose         | `LazyVerticalGrid`           | `FlowRow`                       |
//!
//! Each child keeps its size and its own subtree, painted inside it the way
//! the target paints any other node.

use crate::{fmt_num, node_children, node_hidden, node_size};
use jian_ops_schema::node::{
    ContainerProps, GridTrack, LayoutMode, NumberOrExpression, PenNode, ScriptNode, TrackSize,
};

/// How a container lays its children out on its own.
pub(super) enum FlowKind {
    /// `layout: grid` with these column tracks.
    Grid(Vec<GridTrack>),
    /// A horizontal row with `wrap: true`.
    Wrap,
}

pub(super) struct Flow<'a> {
    pub kind: FlowKind,
    pub children: Vec<&'a PenNode>,
    /// Horizontal space between cells / items.
    pub column_gap: f64,
    /// Vertical space between rows / lines.
    pub row_gap: f64,
}

fn container(node: &PenNode) -> Option<&ContainerProps> {
    match node {
        PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => Some(&n.container),
        PenNode::Group(n) => Some(&n.container),
        PenNode::Rectangle(n) => Some(&n.container),
        _ => None,
    }
}

fn number(v: Option<&NumberOrExpression>) -> f64 {
    match v {
        Some(NumberOrExpression::Number(n)) => *n,
        _ => 0.0,
    }
}

/// The node's flow layout, when it has one and children to flow.
pub(super) fn flow_of(node: &PenNode) -> Option<Flow<'_>> {
    let c = container(node)?;
    let kind = match c.layout {
        Some(LayoutMode::Grid) => {
            let tracks = c.columns.as_ref().map(|t| t.tracks()).unwrap_or_default();
            FlowKind::Grid(if tracks.is_empty() {
                vec![GridTrack::FR1]
            } else {
                tracks
            })
        }
        Some(LayoutMode::Horizontal) if c.wraps() => FlowKind::Wrap,
        _ => return None,
    };
    let children: Vec<&PenNode> = node_children(node)
        .iter()
        .filter(|c| !node_hidden(c))
        .collect();
    if children.is_empty() {
        return None;
    }
    Some(Flow {
        kind,
        children,
        column_gap: number(c.column_gap_value()),
        row_gap: number(c.row_gap_value()),
    })
}

impl Flow<'_> {
    /// Column count of a grid (1 for a wrap).
    pub fn column_count(&self) -> usize {
        match &self.kind {
            FlowKind::Grid(t) => t.len().max(1),
            FlowKind::Wrap => 1,
        }
    }

    /// CSS declarations (`;`-joined, no leading separator) for the
    /// container: display, tracks and gaps.
    pub fn css(&self) -> String {
        let gaps = format!(
            "row-gap:{}px;column-gap:{}px",
            fmt_num(self.row_gap),
            fmt_num(self.column_gap)
        );
        match &self.kind {
            FlowKind::Grid(tracks) => format!(
                "display:grid;grid-template-columns:{};{gaps}",
                tracks
                    .iter()
                    .map(GridTrack::css)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            FlowKind::Wrap => {
                format!("display:flex;flex-wrap:wrap;align-content:flex-start;{gaps}")
            }
        }
    }

    /// The same as a JSX style-object fragment (`, display: 'grid', …`).
    pub fn jsx(&self) -> String {
        let gaps = format!(
            ", rowGap: {}, columnGap: {}",
            fmt_num(self.row_gap),
            fmt_num(self.column_gap)
        );
        match &self.kind {
            FlowKind::Grid(tracks) => format!(
                ", display: 'grid', gridTemplateColumns: '{}'{gaps}",
                tracks
                    .iter()
                    .map(GridTrack::css)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            FlowKind::Wrap => {
                format!(", display: 'flex', flexWrap: 'wrap', alignContent: 'flex-start'{gaps}")
            }
        }
    }

    /// React Native has no grid: both become a wrapping row.
    pub fn react_native(&self) -> String {
        format!(
            ", flexDirection: 'row', flexWrap: 'wrap', alignContent: 'flex-start', rowGap: {}, columnGap: {}",
            fmt_num(self.row_gap),
            fmt_num(self.column_gap)
        )
    }

    /// Flutter layout widget opening (`children: [` left open).
    pub fn flutter_open(&self) -> String {
        match &self.kind {
            FlowKind::Grid(_) => {
                let (w, h) = self.children.first().map_or((1.0, 1.0), |c| node_size(c));
                let ratio = if w > 0.0 && h > 0.0 { w / h } else { 1.0 };
                format!(
                    "GridView.count(crossAxisCount: {}, mainAxisSpacing: {}, crossAxisSpacing: {}, childAspectRatio: {}, shrinkWrap: true, physics: const NeverScrollableScrollPhysics(), children: [",
                    self.column_count(),
                    fmt_num(self.row_gap),
                    fmt_num(self.column_gap),
                    fmt_num((ratio * 1000.0).round() / 1000.0)
                )
            }
            FlowKind::Wrap => format!(
                "Wrap(spacing: {}, runSpacing: {}, children: [",
                fmt_num(self.column_gap),
                fmt_num(self.row_gap)
            ),
        }
    }

    /// SwiftUI layout view opening (`{` left open).
    pub fn swiftui_open(&self) -> String {
        let columns = match &self.kind {
            FlowKind::Grid(tracks) => tracks
                .iter()
                .map(|t| {
                    let size = match t {
                        GridTrack::Size(TrackSize::Px(v)) => format!(".fixed({})", fmt_num(*v)),
                        GridTrack::MinMax {
                            min: TrackSize::Px(v),
                            ..
                        } => format!(".flexible(minimum: {})", fmt_num(*v)),
                        _ => ".flexible()".to_string(),
                    };
                    format!("GridItem({size}, spacing: {})", fmt_num(self.column_gap))
                })
                .collect::<Vec<_>>()
                .join(", "),
            // No flow layout ships with SwiftUI: adaptive columns as wide
            // as the narrowest item wrap the same way.
            FlowKind::Wrap => {
                let min = self
                    .children
                    .iter()
                    .map(|c| node_size(c).0)
                    .fold(f64::INFINITY, f64::min);
                let min = if min.is_finite() { min } else { 40.0 };
                format!(
                    "GridItem(.adaptive(minimum: {}), spacing: {})",
                    fmt_num(min),
                    fmt_num(self.column_gap)
                )
            }
        };
        format!(
            "LazyVGrid(columns: [{columns}], alignment: .leading, spacing: {}) {{",
            fmt_num(self.row_gap)
        )
    }

    /// Compose layout opening (`{` left open).
    pub fn compose_open(&self) -> String {
        let arrangement = format!(
            "horizontalArrangement = Arrangement.spacedBy({}.dp), verticalArrangement = Arrangement.spacedBy({}.dp)",
            fmt_num(self.column_gap),
            fmt_num(self.row_gap)
        );
        match &self.kind {
            FlowKind::Grid(_) => format!(
                "LazyVerticalGrid(columns = GridCells.Fixed({}), {arrangement}) {{",
                self.column_count()
            ),
            FlowKind::Wrap => format!("FlowRow({arrangement}) {{"),
        }
    }

    /// Whether a Compose grid item needs the `item { }` wrapper.
    pub fn compose_items(&self) -> bool {
        matches!(self.kind, FlowKind::Grid(_))
    }
}

#[cfg(test)]
#[path = "codegen_flow_tests.rs"]
mod tests;
