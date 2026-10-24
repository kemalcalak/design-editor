//! Wrapping text keeps the width layout measured it at (#202).

use crate::{pen_document_to_payload, NodePayload};

fn find<'a>(nodes: &'a [NodePayload], id: &str) -> Option<&'a NodePayload> {
    nodes.iter().find_map(|node| {
        if node.schema_id == id {
            Some(node)
        } else {
            find(&node.children, id)
        }
    })
}

/// Box widths of two wrapping `fill_container` texts sharing a row of
/// `row` px.
fn split_row(row: u32) -> [f32; 2] {
    let src = format!(
        r##"{{
      "version":"1.0",
      "children":[{{
        "type":"frame","id":"row","x":0,"y":0,"width":{row},"height":200,"layout":"horizontal",
        "children":[
          {{"type":"text","id":"a","width":"fill_container","textGrowth":"fixed-width",
           "content":"Kerned label","fontSize":16}},
          {{"type":"text","id":"b","width":"fill_container","textGrowth":"fixed-width",
           "content":"Second label","fontSize":16}}
        ]
      }}]
    }}"##
    );
    let doc = jian_ops_schema::load_str(&src).expect("parse").value;
    let loaded = pen_document_to_payload(&doc);
    let roots = &loaded.payload.pages[0].children;
    ["a", "b"].map(|id| find(roots, id).expect("text").w)
}

#[test]
fn fractional_fill_shares_reach_the_painter_at_every_width() {
    // Odd row widths give each text a .5 px share; taffy rounds one box
    // down and the other up. Both wrapping texts must keep the share.
    for row in (241..=641).step_by(2) {
        let share = row as f32 / 2.0;
        assert_eq!(split_row(row), [share, share], "row {row}");
    }
}

#[test]
fn wrapping_text_keeps_its_fractional_measured_width() {
    // Two `fill_container` texts split a 301 px row: 150.5 px each before
    // taffy rounds the boxes to 150 and 151. The wrapping one keeps 150.5,
    // the width its lines were broken at; auto-width text keeps the box.
    let doc = jian_ops_schema::load_str(
        r##"{
      "version":"1.0",
      "children":[{
        "type":"frame","id":"row","x":0,"y":0,"width":301,"height":100,"layout":"horizontal",
        "children":[
          {"type":"text","id":"wraps","width":"fill_container","content":"Left column text",
           "fontSize":14,"textGrowth":"fixed-width"},
          {"type":"text","id":"auto","width":"fill_container","content":"Right column",
           "fontSize":14}
        ]
      }]
    }"##,
    )
    .expect("parse")
    .value;
    let loaded = pen_document_to_payload(&doc);
    let roots = &loaded.payload.pages[0].children;
    let wraps = find(roots, "wraps").expect("wrapping text");
    let auto = find(roots, "auto").expect("auto text");
    assert_eq!(wraps.w, 150.5);
    assert!(wraps.text_wrap);
    assert_eq!(
        auto.w,
        auto.w.round(),
        "auto-width text keeps the rounded box"
    );
}
