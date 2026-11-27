//! Track lines over a selected `layout: grid` frame (#379).
//!
//! While exactly one grid container is selected, its column and row
//! tracks are outlined and the gaps between them tinted, Figma style, so
//! the cells children snap into are visible. The tracks come from
//! `de_editor_core::grid_geometry` over the laid-out scene, rebuilt each
//! frame for the one selected node (a handful of children). Drawn in
//! screen space under the selection handles; never in exports.

use crate::layout_scene::LayoutScene;
use crate::theme::Theme;
use crate::util::alpha;
use crate::widgets::PaintCx;
use crate::{Point2D, Rect};
use de_editor_core::grid_geometry::{content_box, grid_geometry, GridItem};
use de_editor_core::{EditorState, PenNodeExt};
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::{PenNode, ScriptNode};

/// One selected grid's tracks, in document space.
#[derive(Debug, Clone, PartialEq)]
pub struct GridOverlay {
    /// The content box the tracks sit in.
    content: Rect,
    /// `(start, size)` per column, absolute x.
    columns: Vec<(f32, f32)>,
    /// `(start, size)` per row, absolute y.
    rows: Vec<(f32, f32)>,
}

impl GridOverlay {
    /// The overlay for the single selected node when it is a grid frame.
    pub fn build(state: &EditorState, scene: &LayoutScene) -> Option<Self> {
        if state.selection.set.len() > 1 {
            return None;
        }
        let node = state.selected_node()?;
        let container = match node {
            PenNode::Frame(n) | PenNode::Script(ScriptNode { frame: n, .. }) => &n.container,
            PenNode::Group(n) => &n.container,
            PenNode::Rectangle(n) => &n.container,
            _ => return None,
        };
        if container.layout != Some(LayoutMode::Grid) {
            return None;
        }
        let scene_node = scene.active_page()?.find(node.id_str())?;
        let b = scene_node.bounds;
        let content = content_box(
            container,
            (
                f64::from(b.origin.x),
                f64::from(b.origin.y),
                f64::from(b.size.x),
                f64::from(b.size.y),
            ),
        );
        let kids = node.children().map(Vec::as_slice).unwrap_or_default();
        let items: Vec<GridItem> = scene_node
            .children
            .iter()
            .map(|child| {
                let base = kids
                    .iter()
                    .find(|k| k.id_str() == child.id)
                    .map(|k| k.base());
                GridItem {
                    column: base.and_then(|b| b.grid_column),
                    row: base.and_then(|b| b.grid_row),
                    size: (
                        f64::from(child.bounds.size.x),
                        f64::from(child.bounds.size.y),
                    ),
                }
            })
            .collect();
        let g = grid_geometry(container, (content.2, content.3), true, &items);
        Some(Self {
            content: Rect::xywh(
                content.0 as f32,
                content.1 as f32,
                content.2 as f32,
                content.3 as f32,
            ),
            columns: g
                .columns
                .iter()
                .map(|t| ((content.0 + t.start) as f32, t.size as f32))
                .collect(),
            rows: g
                .rows
                .iter()
                .map(|t| ((content.1 + t.start) as f32, t.size as f32))
                .collect(),
        })
    }

    pub(super) fn paint(
        &self,
        cx: &mut PaintCx<'_>,
        canvas: Rect,
        viewport: &de_editor_core::Viewport,
        theme: &Theme,
    ) {
        let sx = |x: f32| canvas.origin.x + viewport.pan_x + x * viewport.zoom;
        let sy = |y: f32| canvas.origin.y + viewport.pan_y + y * viewport.zoom;
        let (top, bottom) = (
            sy(self.content.origin.y),
            sy(self.content.origin.y + self.content.size.y),
        );
        let (left, right) = (
            sx(self.content.origin.x),
            sx(self.content.origin.x + self.content.size.x),
        );
        let gutter = alpha(theme.primary, 0.10);
        let line = alpha(theme.primary, 0.55);
        // Gutters between tracks.
        for pair in self.columns.windows(2) {
            let (end, start) = (pair[0].0 + pair[0].1, pair[1].0);
            if start > end {
                cx.backend.fill_rect(
                    Rect::xywh(sx(end), top, (start - end) * viewport.zoom, bottom - top),
                    gutter,
                );
            }
        }
        for pair in self.rows.windows(2) {
            let (end, start) = (pair[0].0 + pair[0].1, pair[1].0);
            if start > end {
                cx.backend.fill_rect(
                    Rect::xywh(left, sy(end), right - left, (start - end) * viewport.zoom),
                    gutter,
                );
            }
        }
        // Track edges.
        for (start, size) in &self.columns {
            for x in [*start, start + size] {
                let x = sx(x);
                cx.backend
                    .stroke_line(Point2D::new(x, top), Point2D::new(x, bottom), line, 1.0);
            }
        }
        for (start, size) in &self.rows {
            for y in [*start, start + size] {
                let y = sy(y);
                cx.backend
                    .stroke_line(Point2D::new(left, y), Point2D::new(right, y), line, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_scene::{NodeKind, SceneNode, ScenePage};

    fn state(json: &str) -> EditorState {
        let doc = jian_ops_schema::load_str(json).unwrap().value;
        let mut s = EditorState::from_document(doc);
        s.selection.anchor = de_editor_core::NodeId::new("g");
        s.selection.set = vec![de_editor_core::NodeId::new("g")];
        s
    }

    fn node(id: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        let mut n = SceneNode::leaf(id, NodeKind::Frame);
        n.bounds = Rect::xywh(x, y, w, h);
        n
    }

    /// The scene jian lays the 340 px, 3-column, gap-20, pad-10 grid out
    /// as: 93.33 px tracks, 40 px rows.
    fn scene() -> LayoutScene {
        let t = 280.0 / 3.0;
        let mut g = node("g", 0.0, 0.0, 340.0, 120.0);
        g.children = vec![
            node("a", 10.0, 10.0, t, 40.0),
            node("b", 10.0 + t + 20.0, 10.0, t, 40.0),
            node("c", 10.0 + 2.0 * (t + 20.0), 10.0, t, 40.0),
            node("d", 10.0, 70.0, t, 40.0),
        ];
        LayoutScene {
            pages: vec![ScenePage {
                id: "p".into(),
                name: "P".into(),
                children: vec![g],
                image_thumbs: Default::default(),
            }],
            active_page_index: 0,
        }
    }

    #[test]
    fn a_selected_grid_gets_its_tracks() {
        let s = state(
            r#"{"version":"1.0","children":[{"type":"frame","id":"g","x":0,"y":0,"width":340,"height":"fit_content","layout":"grid","columns":3,"gap":20,"padding":10,"children":[
              {"type":"rectangle","id":"a","width":"fill_container","height":40},
              {"type":"rectangle","id":"b","width":"fill_container","height":40},
              {"type":"rectangle","id":"c","width":"fill_container","height":40},
              {"type":"rectangle","id":"d","width":"fill_container","height":40}]}]}"#,
        );
        let overlay = GridOverlay::build(&s, &scene()).expect("grid overlay");
        assert_eq!(overlay.columns.len(), 3);
        assert_eq!(overlay.rows.len(), 2);
        assert!((overlay.columns[0].0 - 10.0).abs() < 0.5);
        assert!((overlay.columns[1].0 - (10.0 + 280.0 / 3.0 + 20.0)).abs() < 0.5);
        assert!((overlay.rows[1].0 - 70.0).abs() < 0.5, "{overlay:?}");
    }

    #[test]
    fn other_selections_get_none() {
        let s = state(
            r#"{"version":"1.0","children":[{"type":"frame","id":"g","width":100,"height":100,"layout":"vertical"}]}"#,
        );
        assert!(GridOverlay::build(&s, &scene()).is_none());
    }
}
