//! A line keeps its direction from the document through the scene, the
//! canvas geometry, SVG, the structured HTML and PPTX in all four
//! directions, so an arrowhead lands on the end the author drew (#389);
//! closed shapes never carry line ends, and inline SVG keeps a polygon's
//! or path's join and miter limit.

use super::shape::emit_line;
use super::xml::line_element;
use de_editor_core::EditorState;
use de_editor_ui::layout_scene::{
    LayoutScene, SceneNode, SceneStroke, SceneStrokeAlign, StrokeEnd, StrokeStyle,
};
use de_editor_ui::{Color, Point2D};

/// The four directions a line can be drawn in from its start `(100, 100)`.
const DIRECTIONS: [(f64, f64); 4] = [(80.0, 40.0), (-80.0, 40.0), (80.0, -40.0), (-80.0, -40.0)];

/// One board per direction, each holding a plain line `plain{i}` and an
/// arrow line `arrow{i}` (triangle at the end), both from `(100, 100)`.
fn scene() -> LayoutScene {
    let boards: Vec<serde_json::Value> = DIRECTIONS
        .iter()
        .enumerate()
        .map(|(i, (dx, dy))| {
            let line = |id: String, extra: serde_json::Value| {
                let mut stroke = serde_json::json!({
                    "thickness": 2,
                    "fill": [{ "type": "solid", "color": "#000000" }],
                });
                for (key, value) in extra.as_object().unwrap() {
                    stroke[key] = value.clone();
                }
                serde_json::json!({
                    "type": "line", "id": id, "x": 100, "y": 100,
                    "x2": dx, "y2": dy, "stroke": stroke,
                })
            };
            serde_json::json!({
                "type": "frame", "id": format!("board{i}"), "x": 0, "y": 0,
                "width": 300, "height": 300, "layout": "none",
                "children": [
                    line(format!("plain{i}"), serde_json::json!({})),
                    line(format!("arrow{i}"), serde_json::json!({ "strokeEnd": "triangle_arrow" })),
                ],
            })
        })
        .collect();
    let doc: jian_ops_schema::PenDocument =
        serde_json::from_value(serde_json::json!({ "version": "1.0", "children": boards }))
            .expect("doc");
    de_pen_loader::editor_state_to_layout_scene(&EditorState::from_document(doc))
}

fn node<'a>(scene: &'a LayoutScene, id: &str) -> &'a SceneNode {
    scene.pages[0].find(id).expect(id)
}

fn ends(dx: f64, dy: f64) -> (Point2D, Point2D) {
    (
        Point2D::new(100.0, 100.0),
        Point2D::new(100.0 + dx as f32, 100.0 + dy as f32),
    )
}

#[test]
fn the_scene_and_its_arrow_geometry_keep_the_authored_direction() {
    let scene = scene();
    for (i, (dx, dy)) in DIRECTIONS.iter().enumerate() {
        let (start, end) = ends(*dx, *dy);
        for id in [format!("plain{i}"), format!("arrow{i}")] {
            assert_eq!(node(&scene, &id).line_endpoints(), (start, end), "{id}");
        }
        // The arrowhead's tip is the end point, node-local.
        let arrow = node(&scene, &format!("arrow{i}"));
        let origin = Point2D::new(start.x.min(end.x), start.y.min(end.y));
        let tip = format!("M {} {} L", end.x - origin.x, end.y - origin.y);
        let geometry = arrow.stroke_geometry.as_deref().expect("geometry");
        assert!(
            geometry.fill_d.contains(&tip),
            "{i}: {tip} in {}",
            geometry.fill_d
        );
    }
}

#[test]
fn svg_writes_lines_start_to_end_and_the_arrow_on_the_end() {
    let scene = scene();
    for (i, (dx, dy)) in DIRECTIONS.iter().enumerate() {
        let (start, end) = ends(*dx, *dy);
        let id = format!("plain{i}");
        let svg = de_editor_ui::svg_export::serialize_node_svg(&scene, &id).expect("svg");
        let segment = format!(
            r#"x1="{}" y1="{}" x2="{}" y2="{}""#,
            start.x, start.y, end.x, end.y
        );
        assert!(svg.contains(&segment), "{id}: {segment} in {svg}");
        let id = format!("arrow{i}");
        let svg = de_editor_ui::svg_export::serialize_node_svg(&scene, &id).expect("svg");
        let head = &node(&scene, &id).stroke_geometry.as_deref().unwrap().fill_d;
        assert!(svg.contains(head.as_str()), "{id}: {svg}");
        assert!(svg.contains(r#"fill-rule="nonzero""#), "{id}: {svg}");
    }
}

#[test]
fn html_writes_lines_start_to_end_and_the_arrow_on_the_end() {
    let scene = scene();
    for (i, (dx, dy)) in DIRECTIONS.iter().enumerate() {
        let (start, end) = ends(*dx, *dy);
        let body = crate::export_html_structured::board_slide_markup(
            &scene.pages[0],
            &format!("board{i}"),
            "Board".into(),
        )
        .expect("markup")
        .body;
        let segment = format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}""#,
            start.x, start.y, end.x, end.y
        );
        assert!(body.contains(&segment), "{i}: {segment} in {body}");
        let head = &node(&scene, &format!("arrow{i}"))
            .stroke_geometry
            .as_deref()
            .unwrap()
            .fill_d;
        assert!(body.contains(head.as_str()), "{i}: {body}");
    }
}

#[test]
fn pptx_flips_follow_the_direction_and_the_tail_is_the_end() {
    let scene = scene();
    for (i, (dx, dy)) in DIRECTIONS.iter().enumerate() {
        for id in [format!("plain{i}"), format!("arrow{i}")] {
            let mut out = String::new();
            let mut next_id = 2;
            emit_line(
                &mut out,
                node(&scene, &id),
                Point2D::new(0.0, 0.0),
                1.0,
                &mut next_id,
            );
            assert_eq!(out.contains(r#"flipH="1""#), *dx < 0.0, "{id}: {out}");
            assert_eq!(out.contains(r#"flipV="1""#), *dy < 0.0, "{id}: {out}");
            assert_eq!(
                out.contains("<a:tailEnd"),
                id.starts_with("arrow"),
                "{id}: {out}"
            );
            assert!(!out.contains("<a:headEnd"), "{id}: {out}");
        }
    }
}

#[test]
fn closed_shapes_never_write_line_ends() {
    let stroke = SceneStroke {
        color: Color::BLACK,
        width: 2.0,
        sides: None,
        align: SceneStrokeAlign::Center,
        style: StrokeStyle {
            start: Some(StrokeEnd::Diamond),
            end: Some(StrokeEnd::TriangleArrow),
            ..StrokeStyle::default()
        },
    };
    let ln = line_element(stroke, 1.0).expect("ln");
    assert!(!ln.contains("headEnd") && !ln.contains("tailEnd"), "{ln}");
    // The heads did not change the box's cap either (flat, the default).
    assert!(ln.contains(r#"cap="flat""#), "{ln}");
}

#[test]
fn inline_svg_polygons_and_paths_keep_join_and_miter_limit() {
    let stroke = serde_json::json!({
        "thickness": 3, "join": "bevel", "miterLimit": 9,
        "fill": [{ "type": "solid", "color": "#000000" }],
    });
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{
            "type": "frame", "id": "board", "width": 300, "height": 300, "layout": "none",
            "children": [
                { "type": "polygon", "id": "poly", "x": 10, "y": 10, "width": 80,
                  "height": 80, "polygonCount": 5, "stroke": stroke },
                { "type": "path", "id": "path", "x": 120, "y": 10, "width": 80,
                  "height": 80, "d": "M0 0 L80 0 L40 80 Z", "stroke": stroke },
            ],
        }],
    }))
    .expect("doc");
    let scene = de_pen_loader::editor_state_to_layout_scene(&EditorState::from_document(doc));
    let body =
        crate::export_html_structured::board_slide_markup(&scene.pages[0], "board", "B".into())
            .expect("markup")
            .body;
    for tag in ["<polygon", "<path"] {
        let at = body.find(tag).unwrap_or_else(|| panic!("{tag} in {body}"));
        let element = &body[at..at + body[at..].find("/>").expect("element end")];
        assert!(element.contains(r#"stroke-linejoin="bevel""#), "{element}");
        assert!(element.contains(r#"stroke-miterlimit="9""#), "{element}");
    }
}
