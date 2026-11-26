//! How much memory this process holds, as the operating system counts it
//! (#456).
//!
//! Two numbers, read without spawning a process or walking the heap, so a
//! Settings line or a debug tool can ask at any time:
//!
//! - `resident`: pages in physical memory right now (RSS / working set).
//! - `footprint`: what the OS charges the process for — macOS
//!   `phys_footprint` (Activity Monitor's "Memory"), Windows private bytes
//!   (Task Manager's "Memory"), Linux RSS. Memory the allocator still holds
//!   but the OS has compressed or swapped counts here and not in `resident`.
//!
//! Both are `None` where the platform call fails or is not wired (wasm).

/// One reading of this process's memory, in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessMemory {
    /// Pages resident in physical memory.
    pub resident: Option<u64>,
    /// What the OS charges the process for (see the module docs).
    pub footprint: Option<u64>,
}

impl ProcessMemory {
    /// The number to show a person: the footprint where the platform has
    /// one, else the resident size.
    pub fn headline(&self) -> Option<u64> {
        self.footprint.or(self.resident)
    }
}

/// Read this process's memory now.
pub fn current() -> ProcessMemory {
    platform::current()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::ProcessMemory;

    /// `RUSAGE_INFO_V2` from `<sys/resource.h>`.
    const RUSAGE_INFO_V2: i32 = 2;
    /// Word offsets into `rusage_info_v2`: a 16-byte uuid, then u64 fields
    /// in order (user / system time, idle / interrupt wakeups, pageins,
    /// wired size, resident size, phys footprint, …).
    const RESIDENT_WORD: usize = 8;
    const FOOTPRINT_WORD: usize = 9;

    extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
    }

    pub(super) fn current() -> ProcessMemory {
        // `rusage_info_v2` is 16 bytes + 17 words; the buffer is larger so
        // a newer header with more fields still fits.
        let mut info = [0u64; 64];
        let pid = std::process::id() as i32;
        // SAFETY: `proc_pid_rusage` writes at most one `rusage_info_v2`
        // (152 bytes) into the 512-byte buffer it is given.
        let status = unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V2, info.as_mut_ptr()) };
        if status != 0 {
            return ProcessMemory::default();
        }
        ProcessMemory {
            resident: Some(info[RESIDENT_WORD]),
            footprint: Some(info[FOOTPRINT_WORD]),
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::ProcessMemory;

    /// `PROCESS_MEMORY_COUNTERS` from `<psapi.h>`.
    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(
            process: isize,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    pub(super) fn current() -> ProcessMemory {
        let mut counters = ProcessMemoryCounters::default();
        let cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        counters.cb = cb;
        // SAFETY: the pseudo handle needs no closing, and the call writes at
        // most `cb` bytes into `counters`.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, cb) };
        if ok == 0 {
            return ProcessMemory::default();
        }
        ProcessMemory {
            resident: Some(counters.working_set_size as u64),
            footprint: Some(counters.pagefile_usage as u64),
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::ProcessMemory;

    pub(super) fn current() -> ProcessMemory {
        let resident = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix("VmRSS:"))
                    .and_then(|rest| {
                        rest.trim()
                            .trim_end_matches("kB")
                            .trim()
                            .parse::<u64>()
                            .ok()
                    })
            })
            .map(|kib| kib * 1024);
        ProcessMemory {
            resident,
            footprint: resident,
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::ProcessMemory;

    pub(super) fn current() -> ProcessMemory {
        ProcessMemory::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_plausible_size() {
        let memory = current();
        if memory.headline().is_none() {
            return; // platform without a reading
        }
        // Other tests allocate and free concurrently in this process, so
        // only the order of magnitude is checked.
        assert!(memory.resident.unwrap_or(0) > 1 << 20, "{memory:?}");
        assert!(memory.headline().unwrap_or(0) > 1 << 20, "{memory:?}");
        assert!(memory.headline().unwrap_or(0) < 1 << 46, "{memory:?}");
    }
}
