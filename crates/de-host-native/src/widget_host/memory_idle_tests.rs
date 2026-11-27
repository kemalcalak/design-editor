//! The idle memory trim waits for a quiet spell (#456): a pan or zoom tick
//! is use, like an edit, so a trim never lands mid-scroll.

use super::WidgetHostNative;
use de_editor_core::memory_idle::IDLE_TRIM_AFTER_MS;

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn host() -> WidgetHostNative {
    let mut state = de_editor_core::EditorState::new();
    state.chat.minimize();
    state.editor_ui.sidebar_open = false;
    let mut host = WidgetHostNative::new();
    *host.editor_state_mut() = state;
    host.set_now_ms(1_000);
    host.mark_editor_state_dirty();
    host
}

#[test]
fn a_pan_defers_the_idle_trim() {
    let mut host = host();
    let first = host.memory_idle_deadline_ms().expect("armed");
    host.set_now_ms(first - 10);
    assert!(host.apply_pan_gesture(W / 2.0, H / 2.0, 24.0, 0.0, W, H));
    host.set_now_ms(first);
    assert!(
        host.run_idle_memory_trim().is_none(),
        "a pan 10 ms ago keeps the trim off"
    );
    assert_eq!(
        host.memory_idle_deadline_ms(),
        Some(first - 10 + IDLE_TRIM_AFTER_MS)
    );
}

#[test]
fn a_zoom_defers_the_idle_trim() {
    let mut host = host();
    let first = host.memory_idle_deadline_ms().expect("armed");
    host.set_now_ms(first - 10);
    host.apply_pinch_gesture(W / 2.0, H / 2.0, -4.0, W, H);
    host.set_now_ms(first);
    assert!(host.run_idle_memory_trim().is_none());
    assert_eq!(
        host.memory_idle_deadline_ms(),
        Some(first - 10 + IDLE_TRIM_AFTER_MS)
    );
    // A quiet spell after the zoom trims once.
    host.set_now_ms(first - 10 + IDLE_TRIM_AFTER_MS);
    assert!(host.run_idle_memory_trim().is_some());
    assert_eq!(host.memory_idle_deadline_ms(), None);
}

#[test]
fn an_agent_at_work_holds_the_trim_off() {
    let mut host = host();
    let first = host.memory_idle_deadline_ms().expect("armed");
    host.set_now_ms(first);
    host.note_memory_activity();
    assert!(host.run_idle_memory_trim().is_none());
    assert_eq!(
        host.memory_idle_deadline_ms(),
        Some(first + IDLE_TRIM_AFTER_MS)
    );
}
