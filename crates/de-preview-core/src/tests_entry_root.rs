//! Preview opens on the board the person picked (#223): the routed entry
//! in APP MODE, the framed root in workbench mode, and history starts
//! there.

#![cfg(test)]

use super::{test_measure, PreviewSession};

/// Three routed pages; `about` links back to `/`.
const SITE_JSON: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "site",
    "app": { "name": "site", "version": "1", "id": "site" },
    "children": [
        { "type": "frame", "id": "home", "name": "Home", "screen": "/",
          "x": 0, "y": 0, "width": 1280, "height": 900 },
        { "type": "frame", "id": "pricing", "name": "Pricing", "screen": "/pricing",
          "x": 1400, "y": 0, "width": 1280, "height": 900,
          "children": [
              { "type": "frame", "id": "to-about", "x": 20, "y": 10,
                "width": 120, "height": 40,
                "events": { "onTap": [ { "replace": "\"/about\"" } ] } }
          ] },
        { "type": "frame", "id": "about", "name": "About", "screen": "/about",
          "x": 2800, "y": 0, "width": 1280, "height": 900,
          "children": [
              { "type": "frame", "id": "to-home", "x": 20, "y": 10,
                "width": 120, "height": 40,
                "events": { "onTap": [ { "replace": "\"/\"" } ] } }
          ] }
    ]
}"##;

/// Two boards at a width no screen band takes, so no route is wired and
/// the session stays in workbench mode on every build.
const BOARDS_JSON: &str = r##"{
    "version": "1.1", "formatVersion": "1.1", "id": "boards",
    "app": { "name": "boards", "version": "1", "id": "boards" },
    "children": [
        { "type": "frame", "id": "first", "x": 0, "y": 0, "width": 700, "height": 500 },
        { "type": "frame", "id": "second", "x": 800, "y": 0, "width": 700, "height": 500 }
    ]
}"##;

fn enter(json: &str, entry: Option<&str>) -> PreviewSession {
    let doc: jian_ops_schema::PenDocument = serde_json::from_str(json).unwrap();
    PreviewSession::enter_with_host_motion_preference(
        &doc,
        (1400.0, 900.0),
        &Default::default(),
        0,
        entry,
        false,
        false,
        test_measure(),
        de_preview_contracts::PreviewHostCapabilities::none(),
        jian_ops_schema::motion::MotionPreference::Full,
        0,
    )
    .expect("enter preview")
}

fn tap(session: &mut PreviewSession, id: &str, now: &mut u64) {
    *now += 5_000;
    session.set_now_ms(*now);
    let (x, y, w, h) = session.node_rect(id).expect("node laid out");
    session.dispatch_tap(x + w / 2.0, y + h / 2.0);
    *now += 5_000;
    session.reconcile(*now);
}

#[test]
fn app_mode_opens_on_the_picked_screen_with_fresh_history() {
    let session = enter(SITE_JSON, Some("pricing"));
    assert_eq!(session.current_path().as_deref(), Some("/pricing"));
    assert_eq!(
        session.framed_root().map(|(id, _)| id).as_deref(),
        Some("pricing")
    );
    assert!(!session.can_go_back());
    // Every route is still there; the screen pills list the entry first.
    let paths: Vec<String> = session
        .screen_switcher_entries()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(paths, ["/pricing", "/", "/about"]);
}

#[test]
fn history_runs_from_the_picked_screen_and_links_to_root_still_work() {
    let mut session = enter(SITE_JSON, Some("pricing"));
    let mut now = 0;
    tap(&mut session, "to-about", &mut now);
    assert_eq!(session.current_path().as_deref(), Some("/about"));
    tap(&mut session, "to-home", &mut now);
    assert_eq!(session.current_path().as_deref(), Some("/"));
    session.go_back();
    now += 5_000;
    session.reconcile(now);
    session.go_back();
    now += 5_000;
    session.reconcile(now);
    assert_eq!(session.current_path().as_deref(), Some("/pricing"));
    assert!(!session.can_go_back());
}

#[test]
fn an_unknown_or_unrouted_entry_keeps_the_authored_entry() {
    for entry in [None, Some("missing"), Some("to-about")] {
        let session = enter(SITE_JSON, entry);
        assert_eq!(session.current_path().as_deref(), Some("/"), "{entry:?}");
    }
}

#[test]
fn workbench_frames_the_picked_board() {
    let session = enter(BOARDS_JSON, Some("second"));
    assert!(!session.is_app_mode());
    assert_eq!(
        session.framed_root().map(|(id, _)| id).as_deref(),
        Some("second")
    );
    let session = enter(BOARDS_JSON, None);
    assert_eq!(
        session.framed_root().map(|(id, _)| id).as_deref(),
        Some("first")
    );
}
