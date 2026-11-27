//! Debounced allocator pressure relief after large document transients.

use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

/// A relief request, with what to free first.
struct Request {
    reason: &'static str,
    /// Freed on the worker as soon as it arrives (#456: the undo steps an
    /// idle trim dropped; freeing a large board takes tens of ms).
    payload: Option<Box<dyn std::any::Any + Send>>,
    /// Also compact the Windows process heap: only after an idle trim,
    /// never after a page switch, since compaction holds the heap lock.
    compact: bool,
}

type RequestSender = mpsc::Sender<Request>;

/// Schedule one process-wide allocator scan off the UI thread. The worker uses
/// a trailing-edge debounce: every later page switch resets the quiet period,
/// so a burst cannot scan while its final old scene is still dropping. The
/// allocator API never releases live allocations.
pub(crate) fn schedule_relief(reason: &'static str) {
    send(Request {
        reason,
        payload: None,
        compact: false,
    });
}

/// Free `payload` off the UI thread, then schedule the relief, with the
/// Windows heap compaction (the idle memory trim's path).
pub(crate) fn release_then_relieve<T: Send + 'static>(payload: T, reason: &'static str) {
    send(Request {
        reason,
        payload: Some(Box::new(payload)),
        compact: true,
    });
}

fn send(request: Request) {
    static SENDER: OnceLock<RequestSender> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("de-heap-relief".into())
            .spawn(move || relief_worker(rx))
            .expect("spawn allocator pressure-relief worker");
        tx
    });
    // A send fails only if the worker died; the payload then drops here.
    let _ = sender.send(request);
}

fn relief_worker(rx: mpsc::Receiver<Request>) {
    while let Ok(first) = rx.recv() {
        let mut reason = first.reason;
        let mut compact = first.compact;
        drop(first.payload);
        loop {
            match rx.recv_timeout(Duration::from_millis(750)) {
                Ok(newer) => {
                    reason = newer.reason;
                    compact |= newer.compact;
                    drop(newer.payload);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        let started = Instant::now();
        let released = platform_pressure_relief(compact);
        if released > 0 || cfg!(debug_assertions) {
            eprintln!(
                "[memory] allocator relief after {reason}: {released} bytes in {:.1} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_pressure_relief(_compact: bool) -> usize {
    use std::ffi::c_void;

    extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut c_void, goal: usize) -> usize;
    }

    // SAFETY: Apple's malloc API explicitly accepts a null zone to inspect all
    // zones and a zero goal to request maximal relief. It only unmaps pages the
    // allocator already considers free.
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) }
}

/// Windows: after an idle trim only (`compact`), the process heap (Rust's
/// system allocator) coalesces its free blocks and decommits what it can.
/// `HeapCompact` reports the largest free block, not what it released, so
/// this reports 0.
#[cfg(windows)]
fn platform_pressure_relief(compact: bool) -> usize {
    if !compact {
        return 0;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetProcessHeap() -> isize;
        fn HeapCompact(heap: isize, flags: u32) -> usize;
    }
    // SAFETY: the process heap handle needs no closing; `HeapCompact` with
    // no flags only coalesces and decommits free blocks.
    unsafe {
        HeapCompact(GetProcessHeap(), 0);
    }
    0
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_pressure_relief(_compact: bool) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_relief_is_safe_to_request_without_large_allocations() {
        let _released = platform_pressure_relief(false);
    }

    #[test]
    fn a_payload_is_freed_off_the_calling_thread() {
        use std::sync::{Arc, Mutex};
        use std::thread::ThreadId;
        struct Flag(Arc<Mutex<Option<ThreadId>>>);
        impl Drop for Flag {
            fn drop(&mut self) {
                *self.0.lock().expect("lock") = Some(std::thread::current().id());
            }
        }
        let dropped_on = Arc::new(Mutex::new(None));
        release_then_relieve(Flag(Arc::clone(&dropped_on)), "test");
        let deadline = Instant::now() + Duration::from_secs(5);
        while dropped_on.lock().expect("lock").is_none() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        let dropped_on = dropped_on.lock().expect("lock").expect("freed");
        assert_ne!(
            dropped_on,
            std::thread::current().id(),
            "freed on the worker"
        );
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn unsupported_platforms_are_a_noop() {
        assert_eq!(platform_pressure_relief(), 0);
    }
}
