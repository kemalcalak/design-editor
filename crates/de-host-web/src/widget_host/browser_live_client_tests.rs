use de_editor_core::browser_stream::{stream_tier, StreamHost};

use super::*;

fn spec(url: &str, frame_width: u32) -> BrowserStreamSpec {
    let tier = stream_tier(StreamHost::Web);
    BrowserStreamSpec {
        url: url.into(),
        layout_width: 1280,
        layout_height: 800,
        mobile: false,
        page_scale: 1.0,
        frame_width,
        frame_height: tier.max_height,
        max_fps: tier.max_fps,
    }
}

fn start(state: &mut LiveClientState, node_id: &str, url: &str) {
    state.apply(BrowserStreamCommand::Start {
        node_id: node_id.into(),
        spec: spec(url, 512),
    });
}

fn keys(set: &BrowserLiveSet) -> Vec<&str> {
    set.nodes.iter().map(|n| n.key.as_str()).collect()
}

#[test]
fn reconnects_back_off_up_to_a_cap() {
    let delays: Vec<u32> = (0..8).map(reconnect_delay_ms).collect();
    assert_eq!(
        delays,
        [1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]
    );
    assert_eq!(reconnect_delay_ms(u32::MAX), 30_000);
}

#[test]
fn the_set_is_posted_after_hello_and_only_when_it_changes() {
    let mut state = LiveClientState::default();
    assert!(!state.wants_connection());
    start(&mut state, "a", "https://example.com/a");
    assert!(state.wants_connection());
    assert_eq!(state.next_post(), None, "no client id before hello");
    state.on_hello("c1".into());
    let set = state.next_post().unwrap();
    assert_eq!(set.client, "c1");
    assert_eq!(keys(&set), ["a#1"]);
    assert_eq!(set.nodes[0].page.url, "https://example.com/a");
    assert_eq!(state.next_post(), None, "one call at a time");
    state.post_done(true);
    assert_eq!(state.next_post(), None, "unchanged");

    start(&mut state, "b", "https://example.com/b");
    let set = state.next_post().unwrap();
    assert_eq!(keys(&set), ["a#1", "b#2"]);
    // The daemon refused it: the whole set goes again.
    state.post_done(false);
    assert!(state.next_post().is_some());
    state.post_done(true);

    state.apply(BrowserStreamCommand::Stop {
        node_id: "a".into(),
        keep_frame: true,
    });
    assert_eq!(keys(&state.next_post().unwrap()), ["b#2"]);
}

#[test]
fn a_resize_keeps_the_key_and_a_reload_takes_a_new_one() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    state.on_hello("c".into());
    state.next_post();
    state.post_done(true);
    state.apply(BrowserStreamCommand::Update {
        node_id: "a".into(),
        spec: spec("https://example.com", 1024),
        reload: false,
    });
    let set = state.next_post().unwrap();
    assert_eq!(keys(&set), ["a#1"]);
    assert_eq!(set.nodes[0].frame_width, 1024);
    state.post_done(true);
    state.apply(BrowserStreamCommand::Update {
        node_id: "a".into(),
        spec: spec("https://example.com", 1024),
        reload: true,
    });
    assert_eq!(keys(&state.next_post().unwrap()), ["a#2"]);
    assert!(!state.is_current("a", "a#1"));
    assert!(state.is_current("a", "a#2"));
}

#[test]
fn an_inactive_tab_posts_an_empty_set_and_resumes_its_streams() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    state.on_hello("c".into());
    state.next_post();
    state.post_done(true);
    state.set_active(false);
    assert!(!state.wants_connection());
    assert!(state.next_post().unwrap().nodes.is_empty());
    state.post_done(true);
    assert_eq!(
        state.on_frame("a#1", 1, || Some(vec![1])),
        None,
        "no decode while away"
    );
    state.set_active(true);
    // The daemon reopens the page, so it streams under a new key whose
    // frames count from 1 again.
    assert_eq!(keys(&state.next_post().unwrap()), ["a#2"]);
    assert_eq!(
        state.on_frame("a#1", 9, || Some(vec![1])),
        None,
        "old stream"
    );
    assert!(state.on_frame("a#2", 1, || Some(vec![2])).is_some());
    state.set_active(true);
    assert!(state.is_current("a", "a#2"), "staying active keeps the key");
}

#[test]
fn an_unreadable_or_unwanted_frame_is_not_unpacked() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    assert_eq!(state.on_frame("a#1", 1, || None), None, "bad base64");
    assert_eq!(
        state.on_frame("x#7", 1, || panic!("unpacked an unknown stream's frame")),
        None
    );
    assert!(state.on_frame("a#1", 1, || Some(vec![1])).is_some());
    assert_eq!(
        state.on_frame("a#1", 1, || panic!("unpacked an old frame")),
        None
    );
}

#[test]
fn the_event_stream_may_close_once_the_daemon_holds_an_empty_set() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    state.on_hello("c".into());
    state.next_post();
    state.post_done(true);
    assert!(!state.idle());
    state.apply(BrowserStreamCommand::Stop {
        node_id: "a".into(),
        keep_frame: true,
    });
    assert!(!state.idle(), "the daemon still streams");
    assert!(state.next_post().unwrap().nodes.is_empty());
    assert!(!state.idle(), "the empty set is on its way");
    state.post_done(true);
    assert!(state.idle());
    state.on_closed();
    start(&mut state, "b", "https://example.com/b");
    assert_eq!(state.next_post(), None, "waits for the next hello");
}

#[test]
fn a_lost_connection_reposts_after_the_next_hello() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    state.on_hello("c1".into());
    state.next_post();
    state.post_done(true);
    assert!(state.on_frame("a#1", 4, || Some(vec![1])).is_some());
    assert_eq!(state.on_disconnect(), 1_000);
    assert!(!state.connected());
    assert_eq!(state.on_disconnect(), 2_000, "backs off while it fails");
    assert_eq!(state.next_post(), None);
    state.on_hello("c2".into());
    let set = state.next_post().unwrap();
    assert_eq!((set.client.as_str(), keys(&set)), ("c2", vec!["a#1"]));
    state.post_done(true);
    assert_eq!(state.on_disconnect(), 1_000, "a hello resets the backoff");
}

#[test]
fn a_node_decodes_one_frame_at_a_time_and_keeps_only_the_newest() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    assert_eq!(
        state.on_frame("x#9", 1, || Some(vec![0])),
        None,
        "unknown stream"
    );
    let first = state.on_frame("a#1", 1, || Some(vec![1])).unwrap();
    assert_eq!((first.node_id.as_str(), first.seq), ("a", 1));
    assert_eq!(state.on_frame("a#1", 2, || Some(vec![2])), None, "waits");
    assert_eq!(state.on_frame("a#1", 3, || Some(vec![3])), None, "replaces");
    assert_eq!(
        state.on_frame("a#1", 3, || Some(vec![3])),
        None,
        "not again"
    );
    let next = state.decode_done(&first).unwrap();
    assert_eq!((next.seq, next.jpeg.as_slice()), (3, &[3u8][..]));
    assert_eq!(state.decode_done(&next), None);
    assert!(
        state.on_frame("a#1", 4, || Some(vec![4])).is_some(),
        "idle again"
    );
}

#[test]
fn a_reload_mid_decode_does_not_stall_the_new_stream() {
    let mut state = LiveClientState::default();
    start(&mut state, "a", "https://example.com");
    let old = state.on_frame("a#1", 1, || Some(vec![1])).unwrap();
    state.apply(BrowserStreamCommand::Update {
        node_id: "a".into(),
        spec: spec("https://example.com", 512),
        reload: true,
    });
    let fresh = state.on_frame("a#2", 1, || Some(vec![2])).unwrap();
    assert_eq!(fresh.key, "a#2");
    assert_eq!(state.decode_done(&old), None);
    assert!(!state.is_current(&old.node_id, &old.key), "lands as stale");
    assert_eq!(state.decode_done(&fresh), None);
    assert!(state.streams_node("a"));
    state.apply(BrowserStreamCommand::Stop {
        node_id: "a".into(),
        keep_frame: false,
    });
    assert!(!state.streams_node("a"));
    assert!(!state.wants_connection());
}
