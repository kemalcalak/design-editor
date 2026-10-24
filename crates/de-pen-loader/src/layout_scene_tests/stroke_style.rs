//! #89: the scene builder records each stroke's cap / join / dash beside
//! the scene (`stroke_render_style`), since `SceneStroke` has no slot for
//! them, and clears the record once the stroke loses its style.

use super::*;
use de_editor_core::stroke_render_style::{registered, LineCap, LineJoin};

fn doc(stroke_extra: &str) -> String {
    format!(
        r##"{{"version":"1.0.0","children":[
          {{"type":"rectangle","id":"stroke-style-scene-rect","x":0,"y":0,"width":40,"height":20,
            "stroke":{{"thickness":2,"fill":[{{"type":"solid","color":"#000000"}}]{stroke_extra}}}}}
        ]}}"##
    )
}

#[test]
fn an_authored_dash_reaches_the_painter_and_clears_with_the_style() {
    let id = "stroke-style-scene-rect";
    let _ = editor_state_to_layout_scene(&state_from(&doc(
        r#","cap":"square","join":"bevel","dashPattern":[6,3],"dashOffset":2"#,
    )));
    let style = registered(id).expect("dashed stroke is recorded");
    assert_eq!(style.cap, Some(LineCap::Square));
    assert_eq!(style.join, Some(LineJoin::Bevel));
    assert_eq!(style.dash.as_deref(), Some(&[6.0, 3.0][..]));
    assert_eq!(style.dash_offset, 2.0);

    let _ = editor_state_to_layout_scene(&state_from(&doc("")));
    assert!(registered(id).is_none(), "a plain stroke clears the record");
}
