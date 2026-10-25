use super::*;
use de_editor_core::PenNodeExt;
use std::time::{Duration, Instant};

#[test]
fn a_stale_script_runs_on_the_worker_and_lands_on_a_later_pump() {
    let doc = r#"{"version":"1.0.0","children":[
      {"type":"script","id":"s","width":40,"height":20,
       "script":"return [{type: 'rectangle', width: pencil.width, height: pencil.height}];"}]}"#;
    let mut state = EditorState::from_document(jian_ops_schema::load_str(doc).unwrap().value);
    let mut host = ScriptNodeHost::default();
    // The first pump only sends the job: the UI thread never waits on it.
    assert!(!host.pump(&mut state));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !host.pump(&mut state) {
        assert!(Instant::now() < deadline, "script result never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    let children = state.doc.children[0].children().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].width_px(), Some(40.0));
}
