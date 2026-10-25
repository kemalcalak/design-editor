use super::*;
use de_editor_core::PenNodeExt;
use std::time::{Duration, Instant};

const ROW: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"row","width":300,"height":40,"layout":"horizontal","children":[
    {"type":"script","id":"s","width":"fill_container","height":20,
     "script":"return [{type: 'rectangle', width: pencil.width, height: pencil.height}];"}]}]}"#;

fn row_state() -> EditorState {
    EditorState::from_document(jian_ops_schema::load_str(ROW).unwrap().value)
}

/// One frame of the host: land, then send.
fn pump(
    host: &mut ScriptNodeHost,
    state: &mut EditorState,
    scene: &LayoutScene,
    epoch: u64,
) -> bool {
    let changed = host.land(state, scene, epoch);
    host.send_stale(state, scene, epoch);
    changed
}

#[test]
fn a_stale_script_runs_on_the_worker_and_lands_at_its_laid_out_size() {
    let mut state = row_state();
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    let mut host = ScriptNodeHost::default();
    // The first frame only sends the job: the UI thread never waits on it.
    assert!(!pump(&mut host, &mut state, &scene, 0));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pump(&mut host, &mut state, &scene, 0) {
        assert!(Instant::now() < deadline, "script result never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    let script = &state.doc.children[0].children().unwrap()[0];
    let children = script.children().unwrap();
    assert_eq!(children.len(), 1);
    // The fill width comes from the layout of the 300 px row.
    assert_eq!(children[0].width_px(), Some(300.0));
}

#[test]
fn a_run_for_a_replaced_document_is_dropped_and_the_new_one_runs() {
    let mut state = row_state();
    let scene = de_pen_loader::editor_state_to_layout_scene(&state);
    let mut host = ScriptNodeHost::default();
    host.send_stale(&mut state, &scene, 0);
    // The document is replaced (epoch 1) by one with the same node ids
    // before the first run answers.
    let mut state = row_state();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut landed = 0;
    while landed == 0 {
        assert!(
            Instant::now() < deadline,
            "the new document's run never landed"
        );
        if pump(&mut host, &mut state, &scene, 1) {
            landed += 1;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Only the new document's run landed; the old one was dropped, so no
    // second landing follows.
    std::thread::sleep(Duration::from_millis(200));
    assert!(!pump(&mut host, &mut state, &scene, 1));
    assert_eq!(
        state.doc.children[0].children().unwrap()[0]
            .children()
            .unwrap()
            .len(),
        1
    );
}
