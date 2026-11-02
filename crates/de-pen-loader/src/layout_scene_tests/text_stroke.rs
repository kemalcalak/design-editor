//! Outlined text (#282): a text node's stroke reaches the scene with its
//! width, colour, alignment and dash, so the canvas can outline the glyphs.

use super::*;
use jian_scene::layout_scene::SceneStrokeAlign;

fn text_node(stroke: &str) -> SceneNode {
    let src = format!(
        r##"{{
      "version":"1.0.0",
      "pages":[{{"id":"p","name":"P","children":[
        {{"type":"text","id":"t","x":0,"y":0,"content":"Outlined","fontSize":32,
         "fill":[{{"type":"solid","color":"#111111"}}]{stroke}}}
      ]}}],"children":[]
    }}"##
    );
    let scene = editor_state_to_layout_scene(&state_from(&src));
    scene.pages[0]
        .children
        .iter()
        .find(|n| n.id == "t")
        .expect("text node")
        .clone()
}

#[test]
fn a_text_stroke_reaches_the_scene() {
    let node = text_node(
        r##","stroke":{"thickness":2,"align":"outside","dashPattern":[3,1],
            "fill":[{"type":"solid","color":"#FF00FF"}]}"##,
    );
    let stroke = node.stroke.expect("the stroke survives the load");
    assert_eq!(stroke.width, 2.0);
    assert_eq!(stroke.align, SceneStrokeAlign::Outside);
    assert_eq!(
        (stroke.color.r, stroke.color.g, stroke.color.b),
        (1.0, 0.0, 1.0)
    );
    assert_eq!(stroke.style.dash.map(|d| d.lengths()[0]), Some(3.0));
    assert!(node.widget.is_none());
}

#[test]
fn text_without_a_stroke_has_none() {
    assert!(text_node("").stroke.is_none());
    let zero =
        text_node(r##","stroke":{"thickness":0,"fill":[{"type":"solid","color":"#FF00FF"}]}"##);
    assert!(zero.stroke.is_none(), "a zero-width stroke draws nothing");
}

#[test]
fn a_variable_text_stroke_colour_resolves() {
    let src = r##"{
      "version":"1.0.0",
      "variables":{"brand":{"type":"color","value":"#FF00FF"}},
      "pages":[{"id":"p","name":"P","children":[
        {"type":"text","id":"t","x":0,"y":0,"content":"Outlined","fontSize":32,
         "stroke":{"thickness":2,"fill":[{"type":"solid","color":"$brand"}]}}
      ]}],"children":[]
    }"##;
    let scene = editor_state_to_layout_scene(&state_from(src));
    let node = scene.pages[0]
        .children
        .iter()
        .find(|n| n.id == "t")
        .unwrap();
    let stroke = node.stroke.expect("the variable stroke survives");
    assert_eq!(
        (stroke.color.r, stroke.color.g, stroke.color.b),
        (1.0, 0.0, 1.0)
    );
}

#[test]
fn an_unpainted_text_stroke_outlines_nothing() {
    let none = text_node(r##","stroke":{"thickness":2,"fill":[]}"##);
    assert!(
        none.stroke.is_none(),
        "no paint must not fall back to black"
    );
    let gradient = text_node(
        r##","stroke":{"thickness":2,"fill":[{"type":"linear_gradient","angle":90,
            "stops":[{"offset":0,"color":"#FF0000"},{"offset":1,"color":"#0000FF"}]}]}"##,
    );
    assert!(
        gradient.stroke.is_some(),
        "a gradient stroke still outlines"
    );
    assert!(!gradient.stroke_fill_layers.is_empty());
}
