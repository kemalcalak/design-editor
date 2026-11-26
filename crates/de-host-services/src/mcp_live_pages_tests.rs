//! Narrowed live snapshots (#456): every read `de_mcp::snapshot_pages`
//! narrows answers byte for byte like it does on the whole document, and a
//! narrowed call that turns out to write is refused and run again on the
//! whole document.

use super::tests::fresh_mcp_server;
use super::*;
use de_editor_core::request_snapshot::{narrowed_snapshot, shared_snapshot_pages};
use de_editor_core::request_snapshot_pages::{kept_pages, PageNeeds};

/// Pages: components (masters, one using a variable), two screens with
/// instances (overrides, a nested instance in an override), a page whose
/// instance targets a plain node elsewhere, that page, and unrelated pages.
fn state() -> EditorState {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(
        r##"{"version":"1.0","children":[],
        "variables":{"brand":{"type":"color","value":"#3355FF"},"gap":{"type":"number","value":12}},
        "pages":[
          {"id":"components","name":"Components","children":[
            {"type":"frame","id":"card","name":"Card","reusable":true,"x":0,"y":0,"width":200,"height":80,
             "layout":"horizontal","gap":"$gap","padding":8,"fill":[{"type":"solid","color":"$brand"}],"children":[
              {"type":"rectangle","id":"card-dot","width":16,"height":16},
              {"type":"text","id":"card-label","content":"Card","fontSize":14}]},
            {"type":"frame","id":"badge","name":"Badge","reusable":true,"x":300,"y":0,"width":60,"height":24,"children":[
              {"type":"text","id":"badge-label","content":"New","fontSize":12}]}]},
          {"id":"screen","name":"Screen","children":[
            {"type":"frame","id":"s-root","name":"Home","x":0,"y":0,"width":480,"height":600,"layout":"vertical","gap":16,"clipContent":true,"children":[
              {"type":"ref","id":"s-card","ref":"card","descendants":{"card-label":{"content":"Hello"}}},
              {"type":"ref","id":"s-card-2","ref":"card","descendants":{
                "card-dot":{"type":"ref","id":"s-nested","ref":"badge"}}},
              {"type":"text","id":"s-title","content":"Title","fontSize":24,"fill":[{"type":"solid","color":"$brand"}]},
              {"type":"rectangle","id":"s-wide","width":900,"height":40}]},
            {"type":"frame","id":"s-second","x":600,"y":0,"width":300,"height":300}]},
          {"id":"legacy-screen","name":"Legacy screen","children":[
            {"type":"frame","id":"l-root","x":0,"y":0,"width":300,"height":300,"children":[
              {"type":"ref","id":"l-inst","ref":"plain-target"}]}]},
          {"id":"legacy","name":"Legacy","children":[
            {"type":"frame","id":"plain-target","x":0,"y":0,"width":120,"height":60,"children":[
              {"type":"text","id":"plain-label","content":"Plain"}]}]},
          {"id":"other","name":"Other","children":[
            {"type":"frame","id":"o-root","x":40,"y":40,"width":300,"height":200,"children":[
              {"type":"text","id":"o-text","content":"Other page"}]}]},
          {"id":"unrelated","name":"Unrelated","children":[
            {"type":"rectangle","id":"u","x":0,"y":0,"width":50,"height":50}]}
        ]}"##,
    )
    .expect("document");
    let mut state = EditorState::from_document(doc);
    state.ui.active_page_index = 1;
    state
}

fn call(id: usize, tool: &str, args: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#
    )
}

/// Every narrowed call shape the live server serves from a partial snapshot.
fn narrowed_calls() -> Vec<String> {
    let calls: Vec<(&str, &str)> = vec![
        ("get_history_depth", "{}"),
        ("get_viewport", "{}"),
        ("get_variables", "{}"),
        ("list_variables", "{}"),
        ("get_active_theme", "{}"),
        ("snapshot_layout", r#"{"maxDepth":5}"#),
        ("snapshot_layout", r#"{"maxDepth":3,"parentId":"s-root"}"#),
        (
            "snapshot_layout",
            r#"{"maxDepth":3,"pageId":"legacy-screen"}"#,
        ),
        ("snapshot_layout", r#"{"pageId":"no-such-page"}"#),
        ("read_nodes", r#"{"nodeIds":["s-card"],"depth":-1}"#),
        (
            "read_nodes",
            r#"{"nodeIds":["s-card","s-card-2/card-label"],"resolveInstances":true,"resolveVariables":true,"depth":-1}"#,
        ),
        (
            "read_nodes",
            r#"{"pageId":"legacy-screen","resolveInstances":true,"depth":-1}"#,
        ),
        (
            "read_nodes",
            r#"{"pageId":"other","includeVariables":true}"#,
        ),
        ("batch_get", r#"{"nodeIds":["s-root"],"readDepth":3}"#),
        (
            "batch_get",
            r#"{"patterns":[{"type":"text"}],"resolveRefs":true}"#,
        ),
        ("batch_get", r#"{"nodeIds":["s-card/card-label"]}"#),
        ("batch_get", r#"{"pageId":"other","readDepth":2}"#),
        (
            "find_empty_space",
            r#"{"width":100,"height":100,"direction":"right"}"#,
        ),
        (
            "find_empty_space",
            r#"{"width":10,"height":10,"direction":"bottom","nodeId":"o-root","pageId":"other"}"#,
        ),
        ("get_screenshot", r#"{"nodeId":"s-root"}"#),
        (
            "execute",
            r#"{"script":"const n = Get(\"s-card\", {depth: -1, resolveInstances: true, resolveVariables: true}); Print(JSON.stringify(n));"}"#,
        ),
        (
            "execute",
            r#"{"script":"const rows = Get(\"s-root\", (node, ctx) => [node.id, ctx.bounds, ctx.problems]); Print(JSON.stringify(rows)); Print(JSON.stringify(GetVariables()));"}"#,
        ),
        (
            "execute",
            r#"{"script":"Print(JSON.stringify(Get(\"l-inst\", {resolveInstances: true, depth: -1})));","pageId":"legacy-screen"}"#,
        ),
        (
            "execute",
            r#"{"script":"Print(JSON.stringify(FindEmptySpace({width: 50, height: 50, direction: 'right'})));"}"#,
        ),
    ];
    calls
        .into_iter()
        .enumerate()
        .map(|(i, (tool, args))| call(i + 1, tool, args))
        .collect()
}

fn answer(state: &mut EditorState, line: &str) -> String {
    crate::mcp_serve::process_message_with_write_applier(state, line, |_, _, _| {
        panic!("a narrowed read never writes: {line}")
    })
    .expect("dispatch")
    .expect("response")
}

#[test]
fn narrowed_reads_answer_like_the_whole_document() {
    let mut live = state();
    let needs = PageNeeds::of(&live);
    for line in narrowed_calls() {
        let pages = de_mcp::snapshot_pages::for_request(&line);
        assert_ne!(
            pages,
            de_mcp::snapshot_pages::SnapshotPages::All,
            "{line} is narrowed"
        );
        let whole = answer(&mut narrowed_snapshot(&mut live), &line);
        let keep = kept_pages(&live, &pages, &needs);
        let mut partial = shared_snapshot_pages(&mut live, None, keep.as_deref()).into_state();
        let narrowed = answer(&mut partial, &line);
        assert_eq!(narrowed, whole, "{line}");
        assert!(
            !whole.contains("\"isError\":true") || line.contains("no-such-page"),
            "{line}: {whole}"
        );
    }
}

#[test]
fn a_narrowed_page_read_leaves_unrelated_pages_out() {
    let live = state();
    let keep = kept_pages(
        &live,
        &de_mcp::snapshot_pages::for_request(&call(1, "snapshot_layout", "{}")),
        &PageNeeds::of(&live),
    )
    .expect("narrowed");
    // components, screen (active), legacy (a plain ref target).
    assert_eq!(keep, vec![true, true, false, true, false, false]);
}

#[test]
fn a_narrowed_call_that_writes_is_refused_and_run_on_the_whole_document() {
    let (req_tx, req_rx) = mpsc::channel();
    let (stop_tx, _stop_rx) = mpsc::channel();
    let mut server = fresh_mcp_server(req_rx, stop_tx);
    let mut live = state();
    // `Insert` is reached without naming it, so the source check reads the
    // script as a read; the narrowed run must refuse the write.
    let line = call(
        1,
        "execute",
        r#"{"script":"globalThis[\"Ins\" + \"ert\"](\"s-root\", {type: \"rectangle\", width: 10, height: 10});"}"#,
    );
    assert_ne!(
        de_mcp::snapshot_pages::for_request(&line),
        de_mcp::snapshot_pages::SnapshotPages::All
    );
    let done = Arc::new(AtomicBool::new(false));
    let client_done = Arc::clone(&done);
    let client = thread::spawn(move || {
        let wake_ui: UiWake = Arc::new(|| {});
        let shared = LiveShared::default();
        let reply = connection::serve_stateful(&line, &req_tx, &shared, &wake_ui);
        client_done.store(true, Ordering::Release);
        reply
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done.load(Ordering::Acquire) && Instant::now() < deadline {
        server.pump(&mut live);
        thread::sleep(Duration::from_millis(1));
    }
    let reply = client.join().expect("client").expect("reply");
    assert!(!reply.contains("\"isError\":true"), "{reply}");
    let pages = live.doc.pages.as_ref().expect("pages");
    let root = pages[1]
        .children
        .iter()
        .find(|node| de_editor_core::PenNodeExt::id_str(*node) == "s-root")
        .expect("root");
    let children = de_editor_core::PenNodeExt::children(root).expect("children");
    assert_eq!(
        children.len(),
        5,
        "the write landed once, on the whole document, beside the four children"
    );
    // Nothing else on the page was lost to a partial snapshot.
    assert_eq!(pages.len(), 6);
    assert!(pages.iter().all(|page| !page.children.is_empty()));
}
