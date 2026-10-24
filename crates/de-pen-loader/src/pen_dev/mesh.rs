//! pen.dev mesh gradient fills for the conversion.
//!
//! pen.dev stores a `columns` × `rows` grid of colours (row-major) plus an
//! optional point per vertex: a moved position or bezier handles. The
//! canonical `MeshGradientBody` is a uniform grid of `{ row, col, color }`
//! stops, so the colours carry over and moved points or handles are
//! reported as an approximation. Colour variables resolve to the node
//! theme's value: a mesh stop cannot hold a reference.

use serde_json::{json, Map, Value};

use super::nodes::Ctx;
use super::variables::{self, Theme};

/// The canonical schema's lattice bounds (`mesh_lattice.rs`).
const MIN_LINES: u64 = 2;
const MAX_LINES: u64 = 5;

/// How far a vertex may sit from its grid position and still count as
/// unmoved.
const POSITION_EPSILON: f64 = 1e-3;

pub(super) fn convert(
    mut obj: Map<String, Value>,
    theme: &Theme,
    ctx: &mut Ctx<'_>,
) -> Option<Value> {
    let lines = |key: &str| obj.get(key).and_then(Value::as_u64).unwrap_or(MIN_LINES);
    let (cols, rows) = (lines("columns"), lines("rows"));
    if !(MIN_LINES..=MAX_LINES).contains(&cols) || !(MIN_LINES..=MAX_LINES).contains(&rows) {
        ctx.report.note("mesh gradient (grid too large)");
        return None;
    }
    let colors = match obj.remove("colors") {
        Some(Value::Array(colors)) => colors,
        _ => Vec::new(),
    };
    let mut stops = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let index = (row * cols + col) as usize;
            let color = colors
                .get(index)
                .and_then(|color| resolve_color(color, theme, ctx))
                .unwrap_or_else(|| "#00000000".to_owned());
            stops.push(json!({ "row": row, "col": col, "color": color }));
        }
    }
    if let Some(Value::Array(points)) = obj.get("points") {
        if !points_on_grid(points, cols, rows) {
            ctx.report.note("mesh gradient points (approximated)");
        }
    }
    let mut out = Map::new();
    out.insert("type".to_owned(), Value::String("mesh_gradient".to_owned()));
    out.insert("rows".to_owned(), json!(rows));
    out.insert("cols".to_owned(), json!(cols));
    out.insert("stops".to_owned(), Value::Array(stops));
    for key in ["opacity", "blendMode"] {
        if let Some(value) = obj.remove(key) {
            out.insert(key.to_owned(), value);
        }
    }
    Some(Value::Object(out))
}

fn resolve_color(color: &Value, theme: &Theme, ctx: &mut Ctx<'_>) -> Option<String> {
    let text = color.as_str()?;
    let Some(name) = variables::reference(text) else {
        return Some(text.to_owned());
    };
    let value = ctx.variables.value(name, theme)?;
    ctx.report.note("paint variable (fixed value)");
    value.as_str().map(str::to_owned)
}

/// Whether every point sits at its uniform grid position with no handles.
fn points_on_grid(points: &[Value], cols: u64, rows: u64) -> bool {
    points.iter().enumerate().all(|(index, point)| {
        let index = index as u64;
        let (col, row) = (index % cols, index / cols);
        let expected = [
            col as f64 / (cols - 1) as f64,
            row as f64 / (rows - 1) as f64,
        ];
        let position = match point {
            Value::Array(_) => Some(point),
            Value::Object(obj) => {
                let has_handles = obj.keys().any(|key| key.ends_with("Handle"));
                if has_handles {
                    return false;
                }
                obj.get("position")
            }
            _ => None,
        };
        let Some(Value::Array(xy)) = position else {
            return true;
        };
        xy.iter().zip(expected).all(|(actual, expected)| {
            actual
                .as_f64()
                .is_some_and(|v| (v - expected).abs() < POSITION_EPSILON)
        })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::pen_dev::{convert, no_assets};

    #[test]
    fn a_mesh_gradient_imports_as_a_uniform_grid() {
        let mut doc = json!({
            "version": "2.21",
            "variables": { "sea": { "type": "color", "value": "#2266AA" } },
            "children": [
                { "type": "rectangle", "id": "plain", "width": 10, "height": 10, "fill": {
                    "type": "mesh_gradient", "columns": 2, "rows": 2, "opacity": 0.5,
                    "colors": ["#FF0000", "$sea", "#00FF00", "#0000FF"],
                    "points": [[0, 0], [1, 0], [0, 1], [1, 1]]
                }},
                { "type": "rectangle", "id": "moved", "width": 10, "height": 10, "fill": {
                    "type": "mesh_gradient", "columns": 2, "rows": 2,
                    "colors": ["#FF0000", "#FF0000", "#00FF00"],
                    "points": [[0, 0], { "position": [1, 0], "leftHandle": [0.1, 0] }, [0, 1], [1, 1]]
                }}
            ]
        });
        let report = convert(&mut doc, &no_assets);
        let fill = &doc["children"][0]["fill"][0];
        assert_eq!(fill["type"], "mesh_gradient");
        assert_eq!(
            (fill["rows"].as_u64(), fill["cols"].as_u64()),
            (Some(2), Some(2))
        );
        assert_eq!(fill["opacity"], 0.5);
        assert_eq!(
            fill["stops"][1],
            json!({ "row": 0, "col": 1, "color": "#2266AA" })
        );
        assert_eq!(fill["stops"][2]["color"], "#00FF00");
        // A missing colour is transparent, as the canvas renders it.
        assert_eq!(
            doc["children"][1]["fill"][0]["stops"][3]["color"],
            "#00000000"
        );
        assert_eq!(
            report
                .unsupported
                .get("mesh gradient points (approximated)"),
            Some(&1)
        );
        assert!(!report.unsupported.contains_key("mesh gradient"));
    }

    #[test]
    fn a_themed_instance_resolves_mesh_colours_under_its_theme() {
        let mut doc = json!({
            "version": "2.21",
            "themes": { "mode": ["light", "dark"] },
            "variables": { "sea": { "type": "color", "value": [
                { "value": "#2266AA" },
                { "value": "#001122", "theme": { "mode": "dark" } }
            ] } },
            "children": [
                { "type": "rectangle", "id": "tile", "reusable": true, "width": 10, "height": 10,
                  "fill": { "type": "mesh_gradient", "columns": 2, "rows": 2,
                              "colors": ["$sea", "$sea", "$sea", "$sea"] } },
                { "type": "ref", "id": "dark-tile", "ref": "tile", "theme": { "mode": "dark" } }
            ]
        });
        convert(&mut doc, &no_assets);
        assert_eq!(
            doc["children"][0]["fill"][0]["stops"][0]["color"],
            "#2266AA"
        );
        let patch = &doc["children"][1]["descendants"]["tile"];
        assert_eq!(patch["fill"][0]["stops"][0]["color"], "#001122");
    }

    #[test]
    fn a_mesh_gradient_loads_into_the_typed_schema() {
        let src = json!({ "version": "2.21", "children": [
            { "type": "rectangle", "id": "r", "width": 10, "height": 10, "fill": {
                "type": "mesh_gradient", "columns": 3, "rows": 2,
                "colors": ["#FF0000", "#00FF00", "#0000FF", "#FFFFFF", "#000000", "#888888"]
            }}
        ]})
        .to_string();
        let loaded = crate::payload::load_canonical_with_assets(&src, &no_assets).expect("loads");
        let doc = serde_json::to_value(&loaded.loaded.value).unwrap();
        assert_eq!(
            doc["children"][0]["fill"][0]["stops"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
    }
}
