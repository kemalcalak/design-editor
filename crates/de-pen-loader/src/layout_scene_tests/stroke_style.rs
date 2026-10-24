//! #89: the scene builder carries each stroke's cap / join / dash on the
//! scene stroke (`SceneStroke::style`).

use super::*;
use jian_scene::layout_scene::{StrokeCap, StrokeJoin};

fn scene_stroke(stroke_extra: &str) -> jian_scene::layout_scene::SceneStroke {
    let src = format!(
        r##"{{"version":"1.0.0","children":[
          {{"type":"rectangle","id":"r","x":0,"y":0,"width":40,"height":20,
            "stroke":{{"thickness":2,"fill":[{{"type":"solid","color":"#000000"}}]{stroke_extra}}}}}
        ]}}"##
    );
    let scene = editor_state_to_layout_scene(&state_from(&src));
    scene.pages[0].children[0].stroke.expect("stroke")
}

#[test]
fn an_authored_dash_reaches_the_scene_stroke() {
    let style =
        scene_stroke(r#","cap":"square","join":"bevel","dashPattern":[6,3],"dashOffset":2"#).style;
    assert_eq!(style.cap, Some(StrokeCap::Square));
    assert_eq!(style.join, Some(StrokeJoin::Bevel));
    let dash = style.dash.expect("dash");
    assert_eq!(dash.lengths(), &[6.0, 3.0]);
    assert_eq!(dash.offset(), 2.0);
}

#[test]
fn a_plain_stroke_carries_a_plain_style() {
    assert!(scene_stroke("").style.is_plain());
}
