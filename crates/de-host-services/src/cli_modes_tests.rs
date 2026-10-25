use super::*;

/// The work a daemon does before its handshake (a fresh editor state and a
/// document with every node kind's arms) fits on the mode thread even when
/// the caller's thread is far smaller than Windows' 1 MiB main thread
/// (#227). Without `on_mode_stack` this aborts the test process.
#[test]
fn a_document_loads_on_the_mode_stack_from_a_small_caller_thread() {
    let dir = std::env::temp_dir().join(format!("de-cli-mode-stack-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stack.de");
    std::fs::write(
        &path,
        r#"{"version":"1.0.0","children":[
          {"type":"frame","id":"f","width":400,"height":300,"layout":"vertical","children":[
            {"type":"text","id":"t","content":"hello"},
            {"type":"script","id":"s","width":100,"height":50,"script":"return [];"},
            {"type":"browser","id":"b","url":"https://example.com","width":320,"height":200}]}]}"#,
    )
    .unwrap();
    let caller = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            on_mode_stack(move || {
                let _fresh = de_editor_core::EditorState::default();
                let state = crate::mcp_serve::load_editor_state(&path).expect("load");
                state.doc.children.len()
            })
        })
        .unwrap();
    assert_eq!(caller.join().unwrap(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_mode_is_left_to_the_caller() {
    assert_eq!(run_cli_mode("prog", "--nope", std::iter::empty()), None);
}

#[test]
fn a_mode_failure_comes_back_from_the_mode_thread() {
    assert_eq!(run_cli_mode("prog", "--mcp", std::iter::empty()), Some(2));
}
