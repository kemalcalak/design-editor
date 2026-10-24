//! Pure [`BrowseHistory`] bookkeeping (no runtime needed).

use super::BrowseHistory;

#[test]
fn new_navigation_after_back_drops_forward_entries() {
    let mut history = BrowseHistory::new("/", 1);
    history.record("/a", 1);
    history.record("/b", 1);
    history.pending = history.step_target(false);
    history.record("/a", 1);
    assert!(history.can_go_forward());
    history.record("/c", 1);
    assert!(!history.can_go_forward(), "a fresh link clears forward");
    assert_eq!(history.entries.len(), 3);
    assert_eq!(history.entries[2].path, "/c");
}

#[test]
fn re_recording_the_current_screen_adds_nothing() {
    let mut history = BrowseHistory::new("/", 1);
    history.record("/", 1);
    assert!(!history.can_go_back());
}

#[test]
fn pending_step_that_lands_elsewhere_is_a_new_entry() {
    let mut history = BrowseHistory::new("/", 1);
    history.record("/a", 1);
    history.pending = history.step_target(false);
    history.record("/z", 1);
    assert_eq!(history.index, 2);
    assert!(!history.can_go_forward());
}

#[test]
fn history_is_capped() {
    let mut history = BrowseHistory::new("/", 1);
    for i in 0..(super::HISTORY_CAP + 10) {
        history.record(&format!("/p{i}"), 1);
    }
    assert_eq!(history.entries.len(), super::HISTORY_CAP);
    assert_eq!(history.index, super::HISTORY_CAP - 1);
    assert!(history.can_go_back());
}
