use super::*;
use de_editor_core::PenNodeExt;
use std::time::{Duration, Instant};

#[test]
fn a_stale_script_runs_on_the_worker_and_lands_at_its_laid_out_size() {
    let doc = r#"{"version":"1.0.0","children":[
      {"type":"frame","id":"row","width":300,"height":40,"layout":"horizontal","children":[
        {"type":"script","id":"s","width":"fill_container","height":20,
         "script":"return [{type: 'rectangle', width: pencil.width, height: pencil.height}];"}]}]}"#;
    let mut state = EditorState::from_document(jian_ops_schema::load_str(doc).unwrap().value);
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    let mut host = ScriptNodeHost::default();
    // The first pump only sends the job: the UI thread never waits on it.
    assert!(!host.pump(&mut state, &scene));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !host.pump(&mut state, &scene) {
        assert!(Instant::now() < deadline, "script result never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    let script = &state.doc.children[0].children().unwrap()[0];
    let children = script.children().unwrap();
    assert_eq!(children.len(), 1);
    // The fill width comes from the layout of the 300 px row.
    assert_eq!(children[0].width_px(), Some(300.0));
}
