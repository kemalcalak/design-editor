//! The latest memory reading the host published (#456).
//!
//! The Settings ▸ System memory row and the `debug_memory_report` tool
//! read one process-wide reading instead of measuring on their own: the
//! desktop host publishes the process footprint and its cache report on
//! a timer (`de-host-desktop` `memory_refresh`, never from paint), and the
//! undo history's estimate when a worker thread has walked it. Every
//! change bumps [`generation`], an atomic read without the lock, so a
//! painter keeps its formatted line until the reading moves and the host
//! repaints only then.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::memory_report::{CacheUsage, MemoryReport};
use crate::process_memory::ProcessMemory;

/// What the host measured last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryReading {
    /// The process as the OS counts it.
    pub process: ProcessMemory,
    /// Every host cache except the undo history (the cheap lines).
    pub caches: MemoryReport,
    /// The undo history's estimate, once a worker has walked it.
    pub history: Option<CacheUsage>,
    /// Whether anything was published yet.
    pub measured: bool,
}

static LATEST: Mutex<Option<MemoryReading>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Apply `change`; bump [`generation`] when the reading moved.
fn update(change: impl FnOnce(&mut Option<MemoryReading>)) {
    let mut latest = LATEST.lock().unwrap_or_else(|poison| poison.into_inner());
    let before = latest.clone();
    change(&mut latest);
    if *latest != before {
        GENERATION.fetch_add(1, Ordering::Release);
    }
}

/// Publish the process reading and the host's cheap cache lines; the last
/// history estimate stays until a new one lands.
pub fn publish_caches(process: ProcessMemory, caches: MemoryReport) {
    update(|latest| {
        let history = latest.as_ref().and_then(|r| r.history.clone());
        *latest = Some(MemoryReading {
            process,
            caches,
            history,
            measured: true,
        });
    });
}

/// Publish the undo history's estimate (from a worker thread).
pub fn publish_history(history: CacheUsage) {
    update(|latest| {
        latest.get_or_insert_with(MemoryReading::default).history = Some(history);
    });
}

/// Drop the history estimate: the host opened or replaced the document,
/// so the old estimate no longer describes its history.
pub fn clear_history() {
    update(|latest| {
        if let Some(reading) = latest {
            reading.history = None;
        }
    });
}

/// The latest reading; `measured` is false before the first publish.
pub fn latest() -> MemoryReading {
    LATEST
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Changes whenever the reading changes; read without the lock.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Forget the whole reading (tests).
pub fn clear() {
    update(|latest| *latest = None);
}

/// `bytes` for a person: `0 KB`, `812 KB`, `14 MB`, `1.2 GB` (binary units,
/// as the OS's own monitors count). A value that would round to 1024 of a
/// unit moves up to the next one, so `1024 MB` never shows.
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if bytes == 0 {
        "0 KB".into()
    } else if (b / MB).round() >= 1024.0 {
        format!("{:.1} GB", b / GB)
    } else if (b / KB).round() >= 1024.0 {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{:.0} KB", (b / KB).max(1.0))
    }
}

/// The Settings row's figures: process memory in use, the host caches'
/// total and the undo history (`None` until measured).
pub fn row_figures(reading: &MemoryReading) -> Option<(String, String, Option<String>)> {
    if !reading.measured {
        return None;
    }
    let used = reading
        .process
        .headline()
        .map(format_bytes)
        .unwrap_or_else(|| "—".into());
    let caches = format_bytes(reading.caches.total_bytes());
    let history = reading.history.as_ref().map(|h| format_bytes(h.bytes));
    Some((used, caches, history))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_the_os_monitors() {
        assert_eq!(format_bytes(0), "0 KB");
        assert_eq!(format_bytes(100), "1 KB");
        assert_eq!(format_bytes(812 * 1024), "812 KB");
        assert_eq!(format_bytes(1_048_500), "1 MB", "not 1024 KB");
        assert_eq!(format_bytes(14 * 1024 * 1024), "14 MB");
        assert_eq!(format_bytes(1_073_000_000), "1023 MB");
        assert_eq!(format_bytes(1_073_500_000), "1.0 GB", "not 1024 MB");
        assert_eq!(format_bytes(1_288_490_189), "1.2 GB");
    }

    #[test]
    fn a_history_estimate_survives_the_next_cache_publish() {
        clear();
        assert!(row_figures(&latest()).is_none(), "nothing measured yet");
        let process = ProcessMemory {
            resident: Some(900 << 20),
            footprint: Some(1 << 30),
        };
        let mut caches = MemoryReport::default();
        caches.push(CacheUsage::exact("decoded_images", 20 << 20, 3, None));
        publish_caches(process, caches.clone());
        assert_eq!(
            row_figures(&latest()),
            Some(("1.0 GB".into(), "20 MB".into(), None))
        );
        publish_history(CacheUsage::estimate("undo_history", 550 << 20, 20, None));
        let before = generation();
        publish_caches(process, caches.clone());
        assert_eq!(generation(), before, "the same reading moves nothing");
        assert_eq!(
            row_figures(&latest()),
            Some(("1.0 GB".into(), "20 MB".into(), Some("550 MB".into())))
        );
        clear_history();
        assert!(generation() > before);
        assert_eq!(latest().history, None, "a new document drops it");
        clear();
    }
}
