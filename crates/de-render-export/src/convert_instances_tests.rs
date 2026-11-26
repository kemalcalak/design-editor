//! Converting hand-built copies into instances (#369) leaves the rendered
//! page pixel-identical and the saved document smaller: a screen with a
//! 30-row hand-built table (zebra rows, a status pill per row, three rows
//! with a different pill colour).

use super::*;
use de_editor_core::repeated_structures::{
    find_repeated_structures, ConvertRequest, DEFAULT_MIN_NODES,
};
use de_editor_core::{ComponentLibrary, EditorState};
use serde_json::{json, Value};

const ROWS: usize = 30;

fn solid(color: &str) -> Value {
    json!([{"type": "solid", "color": color}])
}

fn cell(id: String, text: &str, width: f64, weight: &str) -> Value {
    json!({"type": "text", "id": id, "name": "Cell", "content": text,
           "fontSize": 13, "fontWeight": weight, "fill": solid("#1f2937"),
           "width": width, "height": 18})
}

fn row(index: usize) -> Value {
    let id = format!("row{index}");
    let overdue = index % 10 == 7;
    let status = if overdue {
        "Overdue"
    } else if index % 3 == 0 {
        "Pending"
    } else {
        "Paid"
    };
    json!({
        "type": "frame", "id": id, "name": format!("Row {}", index + 1),
        "layout": "horizontal", "gap": 16, "padding": [10, 16], "alignItems": "center",
        "width": "fill_container", "height": 40, "cornerRadius": 6,
        "fill": solid(if index % 2 == 0 { "#ffffff" } else { "#f9fafb" }),
        "children": [
            cell(format!("{id}-name"), &format!("Invoice #{:04}", 1000 + index * 7), 160.0, "600"),
            cell(format!("{id}-client"), ["Acme Corp", "Globex", "Initech", "Umbrella"][index % 4], 140.0, "400"),
            cell(format!("{id}-date"), &format!("2026-{:02}-{:02}", index % 12 + 1, index % 28 + 1), 100.0, "400"),
            cell(format!("{id}-amount"), &format!("${}.00", 120 + index * 35), 90.0, "500"),
            {"type": "frame", "id": format!("{id}-pill"), "name": "Status",
             "layout": "horizontal", "padding": [2, 8], "cornerRadius": 999,
             "width": "fit_content", "height": "fit_content",
             "fill": solid(if overdue { "#fee2e2" } else { "#dcfce7" }),
             "children": [{"type": "text", "id": format!("{id}-status"), "content": status,
                "fontSize": 11, "fontWeight": "600",
                "fill": solid(if overdue { "#991b1b" } else { "#166534" })}]}
        ]
    })
}

fn screen() -> EditorState {
    let rows: Vec<Value> = (0..ROWS).map(row).collect();
    let table = json!({
        "type": "frame", "id": "screen", "name": "Invoices", "x": 0, "y": 0,
        "width": 720, "height": "fit_content", "layout": "vertical", "gap": 4,
        "padding": 24, "fill": solid("#f3f4f6"),
        "children": [
            {"type": "text", "id": "title", "content": "Invoices", "fontSize": 22,
             "fontWeight": "700", "fill": solid("#111827")},
            {"type": "frame", "id": "table", "name": "Table", "layout": "vertical", "gap": 2,
             "width": "fill_container", "height": "fit_content", "children": rows}
        ]
    });
    let mut state = EditorState::new();
    let mut doc = serde_json::to_value(&state.doc).expect("doc");
    doc["children"] = json!([]);
    doc["pages"] = json!([{"id": "page", "name": "Page 1", "children": [table]}]);
    state.doc = serde_json::from_value(doc).expect("document");
    state.components = ComponentLibrary::from_document(&state.doc);
    state
}

fn render(state: &EditorState) -> Vec<u8> {
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(state);
    let page = scene.active_page().expect("active page");
    render_page_raster_bytes(page, RasterFormat::Png, 1.0).expect("render")
}

#[test]
fn a_thirty_row_table_converts_pixel_identical_and_smaller() {
    let mut state = screen();
    let before_png = render(&state);
    let before_bytes = serde_json::to_vec(&state.doc).unwrap().len();

    let groups = find_repeated_structures(&state.doc, &state.components, None, DEFAULT_MIN_NODES);
    let members: usize = groups.iter().map(|group| group.node_ids.len()).sum();
    assert_eq!(members, ROWS, "every row is in a group: {groups:?}");
    let mut converted = 0;
    for group in &groups {
        let plan = state
            .convert_to_instances(&ConvertRequest {
                node_ids: group.node_ids.clone(),
                ..Default::default()
            })
            .expect("group converts");
        assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
        converted += plan.converted.len();
    }
    assert_eq!(converted, ROWS);

    let after_png = render(&state);
    let after_bytes = serde_json::to_vec(&state.doc).unwrap().len();
    eprintln!(
        "#369 30-row table: {} groups, document {before_bytes} -> {after_bytes} bytes \
         ({:.1}% smaller), page PNG {} bytes before / {} after",
        groups.len(),
        100.0 * (before_bytes - after_bytes) as f64 / before_bytes as f64,
        before_png.len(),
        after_png.len()
    );
    assert!(before_png == after_png, "the rendered page changed");
    assert!(
        after_bytes < before_bytes * 3 / 4,
        "{before_bytes} -> {after_bytes}"
    );
}
