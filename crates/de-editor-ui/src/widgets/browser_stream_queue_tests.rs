//! The live-stream scheduling, driven by hand. Node ids are unique per
//! test and the tests hold the paint registry's lock (its visible set is
//! process-wide).

use de_editor_core::browser_stream::{StreamHost, DESKTOP_STREAM_FPS, MAX_STREAM_HEIGHT};
use de_editor_core::{EditorState, NodeId};

use super::{BrowserStreamCommand, BrowserStreamQueue};
use crate::widgets::browser_render_queue::BrowserRenderQueue;
use crate::widgets::browser_textures::{self, lock_for_tests};

fn state_with(ids: &[&str]) -> EditorState {
    let children: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            serde_json::json!({ "type": "browser", "id": id, "width": 400, "height": 300,
                                "url": "https://example.com" })
        })
        .collect();
    let doc = serde_json::from_value(serde_json::json!({ "version": "1.0", "children": children }))
        .expect("document");
    let mut state = EditorState::new();
    state.replace_document(doc);
    state
}

struct Rig {
    state: EditorState,
    snapshots: BrowserRenderQueue,
    streams: BrowserStreamQueue,
}

impl Rig {
    fn new(ids: &[&str]) -> Self {
        Self {
            state: state_with(ids),
            snapshots: BrowserRenderQueue::new(2),
            streams: BrowserStreamQueue::new(StreamHost::Desktop),
        }
    }

    fn select(&mut self, ids: &[&str]) {
        self.state.set_single_selection(NodeId::new(ids[0]));
        self.state
            .selection
            .set
            .extend(ids[1..].iter().map(|id| NodeId::new(*id)));
    }

    /// Paint a canvas frame showing `shown` (id, on-screen width, height),
    /// then pump both queues.
    fn frame(&mut self, shown: &[(&str, f32, f32)], active: bool) -> Vec<BrowserStreamCommand> {
        browser_textures::begin_frame(false);
        for (id, w, h) in shown {
            browser_textures::note_visible(id, [*w, *h]);
        }
        self.pump(active)
    }

    /// Pump without a new canvas frame.
    fn pump(&mut self, active: bool) -> Vec<BrowserStreamCommand> {
        self.snapshots.pump(&mut self.state, 0.0);
        self.streams.pump(&self.state, &self.snapshots, 2.0, active)
    }
}

fn started(commands: &[BrowserStreamCommand]) -> Vec<&str> {
    let mut ids: Vec<&str> = commands
        .iter()
        .filter_map(|c| match c {
            BrowserStreamCommand::Start { node_id, .. } => Some(node_id.as_str()),
            _ => None,
        })
        .collect();
    ids.sort_unstable();
    ids
}

fn stop(node_id: &str, keep_frame: bool) -> BrowserStreamCommand {
    BrowserStreamCommand::Stop {
        node_id: node_id.into(),
        keep_frame,
    }
}

const SHOWN: [(&str, f32, f32); 1] = [("sq-one", 200.0, 150.0)];

#[test]
fn only_the_selected_node_streams() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-one", "sq-two"]);
    let both = [("sq-one", 200.0, 150.0), ("sq-two", 400.0, 300.0)];
    assert!(rig.frame(&both, true).is_empty(), "nothing selected");
    rig.select(&["sq-one"]);
    let commands = rig.pump(true);
    assert_eq!(started(&commands), ["sq-one"]);
    let BrowserStreamCommand::Start { spec, .. } = &commands[0] else {
        unreachable!()
    };
    // 200 logical px at 2× → 400 device px → the 512 step.
    assert_eq!(
        (spec.frame_width, spec.frame_height),
        (512, MAX_STREAM_HEIGHT)
    );
    assert_eq!(spec.max_fps, DESKTOP_STREAM_FPS);
    // A quiet frame does nothing.
    assert!(rig.pump(true).is_empty());
}

#[test]
fn deselecting_stops_the_stream_and_keeps_its_last_frame() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-one"]);
    rig.select(&["sq-one"]);
    assert_eq!(started(&rig.frame(&SHOWN, true)), ["sq-one"]);
    rig.state.clear_selection();
    assert_eq!(rig.pump(true), [stop("sq-one", true)]);
    assert_eq!(rig.streams.running(), 0);
}

#[test]
fn a_multi_selection_streams_at_most_three_the_largest_first() {
    let _lock = lock_for_tests();
    let ids = ["sq-a", "sq-b", "sq-c", "sq-d"];
    let mut rig = Rig::new(&ids);
    rig.select(&ids);
    let commands = rig.frame(
        &[
            ("sq-a", 400.0, 300.0),
            ("sq-b", 300.0, 200.0),
            ("sq-c", 100.0, 75.0),
            ("sq-d", 50.0, 40.0),
        ],
        true,
    );
    assert_eq!(started(&commands), ["sq-a", "sq-b", "sq-c"]);
    assert_eq!(rig.streams.running(), 3);
}

#[test]
fn an_interactive_node_streams_without_the_selection() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-one"]);
    rig.state
        .editor_ui
        .browser_live
        .set_interactive_node(Some("sq-one"));
    assert_eq!(started(&rig.frame(&SHOWN, true)), ["sq-one"]);
}

#[test]
fn leaving_the_screen_keeps_the_frame_and_a_lost_page_drops_it() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-one"]);
    rig.select(&["sq-one"]);
    rig.frame(&SHOWN, true);
    assert_eq!(rig.frame(&[], true), [stop("sq-one", true)]);
    rig.frame(&SHOWN, true);
    assert!(rig.state.set_selected_browser_url(""));
    assert_eq!(rig.frame(&SHOWN, true), [stop("sq-one", false)]);
}

#[test]
fn an_inactive_window_pauses_on_the_last_frame_and_the_setting_off_drops_it() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-one"]);
    rig.select(&["sq-one"]);
    assert_eq!(started(&rig.frame(&SHOWN, true)), ["sq-one"]);
    assert_eq!(rig.pump(false), [stop("sq-one", true)], "window unfocused");
    assert_eq!(started(&rig.frame(&SHOWN, true)), ["sq-one"], "focus again");
    rig.state.editor_ui.browser_live.set_live_pages(false);
    assert_eq!(rig.pump(true), [stop("sq-one", false)]);
    assert!(rig.frame(&SHOWN, true).is_empty(), "stays off");
    rig.state.editor_ui.browser_live.set_live_pages(true);
    rig.state.editor_ui.browser_live.hold_pages();
    assert!(
        rig.frame(&SHOWN, true).is_empty(),
        "held pages never stream"
    );
}

#[test]
fn zooming_updates_the_frame_size_only_across_a_step() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-zoom"]);
    rig.select(&["sq-zoom"]);
    rig.frame(&[("sq-zoom", 200.0, 150.0)], true);
    assert!(rig.frame(&[("sq-zoom", 210.0, 157.0)], true).is_empty());
    let commands = rig.frame(&[("sq-zoom", 600.0, 450.0)], true);
    assert!(matches!(
        commands.as_slice(),
        [BrowserStreamCommand::Update { spec, reload: false, .. }] if spec.frame_width == 2048
    ));
}

#[test]
fn a_reload_or_a_new_address_reloads_the_stream() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-reload"]);
    rig.select(&["sq-reload"]);
    let shown = [("sq-reload", 200.0, 150.0)];
    rig.frame(&shown, true);
    rig.state.editor_ui.browser_live.reload("sq-reload");
    assert!(matches!(
        rig.frame(&shown, true).as_slice(),
        [BrowserStreamCommand::Update { reload: true, .. }]
    ));
    assert!(rig.state.set_selected_browser_url("https://example.org"));
    assert!(matches!(
        rig.frame(&shown, true).as_slice(),
        [BrowserStreamCommand::Update { reload: true, spec, .. }] if spec.url == "https://example.org"
    ));
}

#[test]
fn a_new_document_stops_the_old_streams() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-old"]);
    rig.select(&["sq-old"]);
    rig.frame(&[("sq-old", 200.0, 150.0)], true);
    let next = state_with(&["sq-new"]);
    rig.state.replace_document(next.doc.clone());
    assert_eq!(rig.pump(true), [stop("sq-old", false)]);
    assert_eq!(rig.streams.running(), 0);
}

#[test]
fn the_preview_streams_its_pages_without_a_selection_at_most_three() {
    let _lock = lock_for_tests();
    let ids = ["sq-pa", "sq-pb", "sq-pc", "sq-pd"];
    let mut rig = Rig::new(&ids);
    rig.state.editor_ui.preview.mode = true;
    let shown = [
        ("sq-pa", 100.0, 75.0),
        ("sq-pb", 400.0, 300.0),
        ("sq-pc", 300.0, 200.0),
        ("sq-pd", 50.0, 40.0),
    ];
    let commands = rig.frame(&shown, true);
    assert_eq!(started(&commands), ["sq-pa", "sq-pb", "sq-pc"]);
    let BrowserStreamCommand::Start { spec, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(spec.max_fps, DESKTOP_STREAM_FPS, "same tier as the canvas");
    assert!(rig.pump(true).is_empty(), "a quiet frame does nothing");
}

#[test]
fn leaving_the_preview_stops_its_streams_and_the_setting_off_keeps_stills() {
    let _lock = lock_for_tests();
    let mut rig = Rig::new(&["sq-prev"]);
    let shown = [("sq-prev", 200.0, 150.0)];
    rig.state.editor_ui.preview.mode = true;
    rig.state.editor_ui.browser_live.set_live_pages(false);
    assert!(rig.frame(&shown, true).is_empty(), "stills only");
    rig.state.editor_ui.browser_live.set_live_pages(true);
    assert_eq!(started(&rig.frame(&shown, true)), ["sq-prev"]);
    rig.state.editor_ui.preview.mode = false;
    assert_eq!(rig.frame(&shown, true), [stop("sq-prev", true)]);
}
