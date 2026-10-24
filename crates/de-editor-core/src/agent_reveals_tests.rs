use super::{recovery_stagger_ms, reveal_stagger_ms, REVEAL_QUEUE_CAP_MS};
use crate::agent_indicators::{REVEAL_FRAME_MS, REVEAL_STAGGER_MS};

#[test]
fn small_batches_keep_the_full_beat() {
    for count in [0, 1, 10, REVEAL_QUEUE_CAP_MS / REVEAL_STAGGER_MS] {
        assert_eq!(reveal_stagger_ms(count), REVEAL_STAGGER_MS, "{count}");
    }
}

#[test]
fn a_large_batch_fits_the_queue_cap() {
    // #17: 301 nodes used to take 301 × 160 ms ≈ 48 s of 60 fps repaints.
    let stagger = reveal_stagger_ms(301);
    assert!(stagger < REVEAL_STAGGER_MS);
    assert!(301 * stagger <= REVEAL_QUEUE_CAP_MS);
}

#[test]
fn a_huge_batch_never_goes_below_one_frame() {
    assert_eq!(reveal_stagger_ms(10_000), REVEAL_FRAME_MS);
}

#[test]
fn burst_recovery_keeps_the_queue_spacing() {
    assert_eq!(recovery_stagger_ms([100, 120, 140].into_iter()), 20);
    assert_eq!(
        recovery_stagger_ms([100, 100, 105].into_iter()),
        REVEAL_FRAME_MS
    );
    assert_eq!(recovery_stagger_ms([100].into_iter()), REVEAL_STAGGER_MS);
    assert_eq!(
        recovery_stagger_ms([0, 1_000].into_iter()),
        REVEAL_STAGGER_MS
    );
}
