//! A tool call held for the user's answer (#398) never ends in a silent
//! timeout, and never hangs once the host lets it go.

use super::*;

#[test]
fn a_held_call_dropped_by_the_host_is_declined_not_hung() {
    let (executor, rx) = chat_tool_channel();
    let worker = std::thread::spawn(move || executor.execute("delete_node", "{}"));
    let req = rx.recv().expect("the call reaches the host");
    req.hold_for_user();
    // Stop / tab or document closed / quit: the session and its deferred
    // calls are dropped without an answer.
    drop(req);
    let result = worker.join().expect("worker ends");
    assert!(result.is_error);
    assert_eq!(
        result.content,
        de_editor_core::agent_permission::DECLINED_RESULT
    );
}

#[test]
fn an_unheld_call_dropped_by_the_host_is_aborted() {
    let (executor, rx) = chat_tool_channel();
    let worker = std::thread::spawn(move || executor.execute("update_node", "{}"));
    drop(rx.recv().expect("the call reaches the host"));
    let result = worker.join().expect("worker ends");
    assert!(result.content.contains("aborted"));
}

#[test]
fn a_held_call_still_gets_its_late_answer() {
    let (executor, rx) = chat_tool_channel();
    let worker = std::thread::spawn(move || executor.execute("delete_node", "{}"));
    let req = rx.recv().expect("the call reaches the host");
    req.hold_for_user();
    req.ack
        .send(ChatToolResult {
            content: "ran".into(),
            is_error: false,
        })
        .unwrap();
    assert_eq!(worker.join().unwrap().content, "ran");
}

#[test]
fn a_call_given_up_on_is_never_held_or_run() {
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let req = ChatToolRequest::new("delete_node", "{}", tx);
    assert!(!req.abandoned());
    // The worker's timeout won the race.
    req.hold.store(GAVE_UP, Ordering::SeqCst);
    assert!(req.abandoned());
    assert!(!req.hold_for_user(), "too late to hold");
}

#[test]
fn requests_get_their_own_ids() {
    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
    let a = ChatToolRequest::new("x", "{}", tx.clone());
    let b = ChatToolRequest::new("x", "{}", tx);
    assert_ne!(a.id, b.id);
    assert!(a.hold_for_user());
    assert!(a.hold_for_user(), "holding twice is fine");
}
