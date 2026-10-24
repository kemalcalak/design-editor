//! Finalize-lifecycle invariant regression tests (0718-1-k3-1 postmortem) —
//! the DESKTOP half. `chat_agent_loop_finalize_tests.rs` (de-host-services)
//! covers the loop-side Err-exit gap; these cover the teardown-side gap:
//! `app_handler.rs` drains New Chat / Stop / Close Tab BEFORE
//! `chat_session::pump` runs each frame, so any of those can discard an
//! in-flight, still-unfinalized design-loop session before `pump`'s own
//! poll-backstop ever gets a chance to see it.

use super::*;
use de_editor_core::PenNodeExt;

/// A roleless "Header" frame directly inserted against `host`'s live state
/// — the same structural proof `chat_session_tests.rs::
/// pump_runs_loop_finalize_backstop_against_live_state` uses
/// (`apply_loop_finalize`'s role inference assigns it `navbar`). Bypasses
/// the tool-channel round trip entirely since these tests exercise the
/// TEARDOWN backstop (a direct, synchronous `apply_loop_finalize` call),
/// not the `LOOP_FINALIZE_OP` forwarding path.
fn insert_roleless_header(host: &mut WidgetHostNative) {
    let (result, _mutated) = de_host_services::design_agent_tools::execute_agent_tool(
        host.editor_state_mut(),
        "insert_node",
        r#"{"kind":"frame","name":"Header","x":"0","y":"0","width":"1200","height":"64"}"#,
    );
    assert!(!result.is_error, "setup: insert_node failed: {result:?}");
}

fn header_role(host: &WidgetHostNative) -> Option<String> {
    host.editor_state()
        .active_children()
        .iter()
        .find(|n| n.base().name.as_deref() == Some("Header"))
        .and_then(|n| n.base().role.clone())
}

/// An unfinalized design-loop session — `is_design_loop()` true,
/// `loop_finalized()` false — built without a live worker (no deltas will
/// ever arrive), matching every teardown path's shape: it only cares about
/// the session's OWN flags, never about whether the turn is still streaming.
/// Its run starts from `host`'s page as it is now, so only roots inserted
/// after this call count as the run's.
fn unfinalized_design_loop_session(host: &WidgetHostNative) -> ChatSession {
    let (_delta_tx, delta_rx) = std::sync::mpsc::channel();
    ChatSession::from_channels(delta_rx, None).into_design_loop(
        de_editor_core::write_scope::WriteBaseline::capture(host.editor_state()),
    )
}

#[test]
fn drain_new_chat_request_runs_the_teardown_backstop_before_dropping_the_session() {
    let _guard = crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    de_editor_core::agent_indicators::clear();

    let mut host = WidgetHostNative::new();
    let session = unfinalized_design_loop_session(&host);
    insert_roleless_header(&mut host);
    let mut current_chat = Some(session);
    let mut current_design = None;
    host.editor_state_mut().chat.pending_new_chat = true;

    let drained = drain_new_chat_request(&mut host, &mut current_chat, &mut current_design);

    assert!(drained);
    assert!(
        current_chat.is_none(),
        "New Chat must still drop the session"
    );
    assert_eq!(
        header_role(&host),
        Some("navbar".to_string()),
        "New Chat must run the finalize backstop on the OLD session before dropping it"
    );
}

#[test]
fn drain_stop_request_runs_the_teardown_backstop_before_dropping_the_session() {
    let _guard = crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    de_editor_core::agent_indicators::clear();

    let mut host = WidgetHostNative::new();
    let session = unfinalized_design_loop_session(&host);
    insert_roleless_header(&mut host);
    let mut current_chat = Some(session);
    let mut current_design = None;
    host.editor_state_mut().chat.pending_stop_chat = true;

    let drained = drain_stop_request(&mut host, &mut current_chat, &mut current_design, None);

    assert!(drained);
    assert!(current_chat.is_none(), "Stop must still drop the session");
    assert_eq!(
        header_role(&host),
        Some("navbar".to_string()),
        "Stop must run the finalize backstop on the OLD session before dropping it"
    );
}

/// Idempotency: a session that already ran finalize (`mark_loop_finalized`)
/// must NOT get a redundant backstop pass on teardown — proves the gate is
/// `is_design_loop() && !loop_finalized()`, not just `is_design_loop()`.
#[test]
fn drain_new_chat_request_skips_the_backstop_when_already_finalized() {
    let _guard = crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    de_editor_core::agent_indicators::clear();

    let mut host = WidgetHostNative::new();
    let mut session = unfinalized_design_loop_session(&host);
    insert_roleless_header(&mut host);
    session.mark_loop_finalized();
    let mut current_chat = Some(session);
    let mut current_design = None;
    host.editor_state_mut().chat.pending_new_chat = true;

    drain_new_chat_request(&mut host, &mut current_chat, &mut current_design);

    assert_eq!(
        header_role(&host),
        None,
        "an already-finalized session must not get a redundant backstop pass \
         (this Header would only gain a role if apply_loop_finalize ran again)"
    );
}

/// A plain (non-design) chat session must never trigger the document-
/// mutating backstop on teardown, mirroring the loop-side
/// `finalize_on_exit` gate's own invariant.
#[test]
fn drain_new_chat_request_skips_the_backstop_for_a_non_design_session() {
    let _guard = crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    de_editor_core::agent_indicators::clear();

    let mut host = WidgetHostNative::new();
    insert_roleless_header(&mut host);
    let (_delta_tx, delta_rx) = std::sync::mpsc::channel();
    let mut current_chat = Some(ChatSession::from_channels(delta_rx, None)); // NOT .into_design_loop()
    let mut current_design = None;
    host.editor_state_mut().chat.pending_new_chat = true;

    drain_new_chat_request(&mut host, &mut current_chat, &mut current_design);

    assert_eq!(
        header_role(&host),
        None,
        "a plain chat session must never run the document-mutating backstop"
    );
}

/// #116: the backstop finalizes only the roots the run created. A Header
/// that was on the page before the run keeps its authored shape (no role),
/// while the run's own Header gets the navbar role.
#[test]
fn teardown_backstop_leaves_a_root_the_run_did_not_create_unchanged() {
    let _guard = crate::agent_indicator_test_lock::LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    de_editor_core::agent_indicators::clear();

    let mut host = WidgetHostNative::new();
    insert_roleless_header(&mut host);
    let hand_made = host
        .editor_state()
        .active_children()
        .iter()
        .find(|n| n.base().name.as_deref() == Some("Header"))
        .cloned()
        .expect("hand-made Header");
    let session = unfinalized_design_loop_session(&host);
    insert_roleless_header(&mut host);
    let mut current_chat = Some(session);
    let mut current_design = None;
    host.editor_state_mut().chat.pending_new_chat = true;

    drain_new_chat_request(&mut host, &mut current_chat, &mut current_design);

    let roots = host.editor_state().active_children();
    let kept = roots
        .iter()
        .find(|n| n.id_str() == hand_made.id_str())
        .expect("hand-made Header");
    assert_eq!(
        serde_json::to_value(kept).expect("serialize"),
        serde_json::to_value(&hand_made).expect("serialize"),
        "a root that was there before the run must stay as authored"
    );
    let generated = roots
        .iter()
        .find(|n| n.base().name.as_deref() == Some("Header") && n.id_str() != hand_made.id_str())
        .expect("the run's Header");
    assert_eq!(generated.base().role.as_deref(), Some("navbar"));
}
