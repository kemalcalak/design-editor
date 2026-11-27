//! `<a:custGeom>` for outlines DrawingML has no preset for (#389):
//! smoothed rect corners, rounded polygons and stars, and plain polygons.
//! The outline is node-local doc px, which is the shape box the geometry's
//! path space is laid over, so coordinates convert straight to EMU.

use de_editor_core::vector_edit::{parse_path_data, segment_controls, segment_count, Subpath};
use de_editor_ui::layout_scene::SceneNode;
use de_editor_ui::Rect;
use std::fmt::Write as _;

use super::units::{emu, emu_extent};

/// The geometry of a box-shaped node: its corner outline when the scene
/// built one, else `None` (the caller writes a preset).
pub fn box_outline_geom(node: &SceneNode, rect: Rect) -> Option<String> {
    let outline = node.corner_path.as_deref()?;
    cust_geom(&parse_path_data(&outline.d)?, rect)
}

/// The geometry of a polygon or star: its rounded outline, or the sharp
/// vertices the canvas paints.
pub fn polygon_geom(node: &SceneNode, rect: Rect) -> Option<String> {
    if let Some(geom) = box_outline_geom(node, rect) {
        return Some(geom);
    }
    let local = Rect::xywh(0.0, 0.0, rect.size.x, rect.size.y);
    let points: Vec<(f64, f64)> = de_editor_ui::polygon_geometry::polygon_node_points(node, local)
        .iter()
        .map(|p| (f64::from(p.x), f64::from(p.y)))
        .collect();
    let mut ring = Subpath::polyline(&points);
    ring.closed = true;
    cust_geom(&[ring], rect)
}

/// `subs` (node-local) as a custom geometry over a `rect`-sized box.
fn cust_geom(subs: &[Subpath], rect: Rect) -> Option<String> {
    let mut paths = String::new();
    for sub in subs.iter().filter(|s| !s.anchors.is_empty()) {
        let _ = write!(
            paths,
            "<a:path w=\"{}\" h=\"{}\">",
            emu_extent(rect.size.x),
            emu_extent(rect.size.y)
        );
        let first = &sub.anchors[0];
        let _ = write!(paths, "<a:moveTo>{}</a:moveTo>", pt((first.x, first.y)));
        for seg in 0..segment_count(sub) {
            let c = segment_controls(sub, seg)?;
            if c[1] == c[0] && c[2] == c[3] {
                let _ = write!(paths, "<a:lnTo>{}</a:lnTo>", pt(c[3]));
            } else {
                let _ = write!(
                    paths,
                    "<a:cubicBezTo>{}{}{}</a:cubicBezTo>",
                    pt(c[1]),
                    pt(c[2]),
                    pt(c[3])
                );
            }
        }
        if sub.closed {
            paths.push_str("<a:close/>");
        }
        paths.push_str("</a:path>");
    }
    if paths.is_empty() {
        return None;
    }
    Some(format!(
        "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>\
<a:rect l=\"0\" t=\"0\" r=\"r\" b=\"b\"/><a:pathLst>{paths}</a:pathLst></a:custGeom>"
    ))
}

fn pt((x, y): (f64, f64)) -> String {
    format!("<a:pt x=\"{}\" y=\"{}\"/>", emu(x as f32), emu(y as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_ui::layout_scene::{NodeKind, SceneCornerPath};
    use std::sync::Arc;

    #[test]
    fn an_outline_becomes_moves_lines_curves_and_a_close() {
        let mut n = SceneNode::leaf("r", NodeKind::Rect);
        n.corner_path = Some(Arc::new(SceneCornerPath {
            d: "M 0 0 L 10 0 C 10 5 10 10 5 10 Z".into(),
            ..SceneCornerPath::default()
        }));
        let xml = box_outline_geom(&n, Rect::xywh(30.0, 40.0, 10.0, 10.0)).expect("geom");
        assert!(xml.starts_with("<a:custGeom>"), "{xml}");
        assert!(
            xml.contains("<a:moveTo><a:pt x=\"0\" y=\"0\"/></a:moveTo>"),
            "{xml}"
        );
        assert!(
            xml.contains("<a:lnTo><a:pt x=\"95250\" y=\"0\"/></a:lnTo>"),
            "{xml}"
        );
        assert!(xml.contains("<a:cubicBezTo>"), "{xml}");
        // The closing edge back to the start is a line, then the close.
        assert!(
            xml.ends_with("<a:close/></a:path></a:pathLst></a:custGeom>"),
            "{xml}"
        );
        assert!(xml.contains("<a:path w=\"95250\" h=\"95250\">"), "{xml}");
        assert!(box_outline_geom(&SceneNode::leaf("p", NodeKind::Rect), Rect::ZERO).is_none());
    }

    #[test]
    fn a_sharp_polygon_writes_its_vertices() {
        let mut n = SceneNode::leaf("p", NodeKind::Polygon);
        n.polygon_sides = 3;
        let xml = polygon_geom(&n, Rect::xywh(0.0, 0.0, 10.0, 10.0)).expect("geom");
        assert_eq!(xml.matches("<a:lnTo>").count(), 3, "{xml}");
        assert!(!xml.contains("cubicBezTo"), "{xml}");
    }
}
