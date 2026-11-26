//! Boolean groups in exports (#388): SVG and HTML write the computed
//! path, the `.de` keeps the group.

use de_editor_core::EditorState;

const DOC: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"f","name":"Board","x":0,"y":0,"width":300,"height":200,
   "fill":[{"type":"solid","color":"#ffffff"}],"children":[
    {"type":"boolean","id":"b","name":"Union","op":"union",
     "fill":[{"type":"solid","color":"#ff0000"}],"children":[
      {"type":"ellipse","id":"c1","x":10,"y":10,"width":100,"height":100},
      {"type":"ellipse","id":"c2","x":70,"y":10,"width":100,"height":100}]}]}]}"##;

fn state() -> EditorState {
    EditorState::from_document(jian_ops_schema::load_str(DOC).expect("parses").value)
}

#[test]
fn svg_writes_one_computed_path_for_a_boolean_group() {
    let scene = de_pen_loader::editor_state_to_active_page_layout_scene(&state());
    let svg = de_editor_ui::svg_export::serialize_active_page_svg(&scene).expect("svg");
    assert!(
        !svg.contains("<ellipse"),
        "the sources are not written: {svg}"
    );
    let paths: Vec<&str> = svg.match_indices("<path").map(|(i, _)| &svg[i..]).collect();
    assert_eq!(paths.len(), 1, "{svg}");
    assert!(paths[0].contains(" C"), "curves kept: {svg}");
    assert!(
        svg.contains("fill=\"rgb(255,0,0)\""),
        "the group fill: {svg}"
    );
    assert!(svg.contains("fill-rule=\"evenodd\""), "{svg}");
}

#[test]
fn html_codegen_writes_the_computed_path() {
    let s = state();
    let baked = de_pen_loader::bake_boolean_groups(&s).expect("has a boolean group");
    let export = de_codegen::export_editor_html(
        &baked,
        de_codegen::HtmlTarget::Css,
        &de_codegen::HtmlExportOptions::default(),
    );
    assert!(export.html.contains("<path"), "{}", export.html);
    assert!(!export.html.contains("<ellipse"), "{}", export.html);
    // The document keeps its boolean group.
    assert!(serde_json::to_string(&s.doc)
        .unwrap()
        .contains("\"boolean\""));
}
