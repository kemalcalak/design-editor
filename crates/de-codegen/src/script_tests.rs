//! A script node exports as a frame holding its generated children (#19):
//! the saved output, never the program.

use super::*;
use jian_ops_schema::node::PenNode;

fn doc() -> jian_ops_schema::PenDocument {
    jian_ops_schema::load_str(
        r##"{"version":"1.0.0","children":[
          {"type":"script","id":"chart","x":0,"y":0,"width":200,"height":80,"clipContent":true,
           "script":"// @input n: number = 2\nreturn [];","scriptHash":"h",
           "children":[
             {"type":"text","id":"chart-0","x":0,"y":0,"content":"Q1 revenue"},
             {"type":"rectangle","id":"chart-1","x":0,"y":30,"width":40,"height":50,
              "fill":[{"type":"solid","color":"#3b82f6"}]}]}]}"##,
    )
    .unwrap()
    .value
}

#[test]
fn generated_children_reach_the_code_and_the_program_does_not() {
    let doc = doc();
    assert!(matches!(doc.children[0], PenNode::Script(_)));
    for code in [Html.generate(&doc), React.generate(&doc)] {
        assert!(code.contains("Q1 revenue"), "code: {code}");
        assert!(!code.contains("@input"), "the program leaked: {code}");
    }
}

#[test]
fn the_html_export_clips_the_script_box() {
    let html = Html.generate(&doc());
    assert!(
        html.contains("overflow: hidden") || html.contains("overflow:hidden"),
        "html: {html}"
    );
}
