//! Bounded background fetcher for the signed-in account's profile avatar.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use de_editor_ui::account_avatar_runtime::{
    complete_account_avatar_request, take_account_avatar_requests, AccountAvatarFetchRequest,
};
use de_host_services::profile_avatar_fetch::{
    fetch_account_avatar_blocking, ProfileAvatarFetchError as AvatarFetchError,
};

const MAX_CONCURRENT_FETCHES: usize = 3;

type Fetcher =
    Arc<dyn Fn(&AccountAvatarFetchRequest) -> Result<Vec<u8>, AvatarFetchError> + Send + Sync>;

struct FetchJob {
    request: AccountAvatarFetchRequest,
    rx: Receiver<Result<Vec<u8>, AvatarFetchError>>,
}

pub(crate) struct AccountAvatarHost {
    jobs: Vec<FetchJob>,
    fetcher: Fetcher,
}

#[cfg(test)]
static AVATAR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Serialize every test that touches the process-global account avatar slot.
#[cfg(test)]
pub(crate) fn lock_avatar_test_registry() -> std::sync::MutexGuard<'static, ()> {
    AVATAR_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

impl AccountAvatarHost {
    pub(crate) fn new() -> Self {
        Self::with_fetcher(Arc::new(|request| {
            fetch_account_avatar_blocking(request.url())
        }))
    }

    fn with_fetcher(fetcher: Fetcher) -> Self {
        Self {
            jobs: Vec::new(),
            fetcher,
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Drain results, then fill only the bounded free worker slots.
    pub(crate) fn pump(&mut self) -> bool {
        let mut changed = self.poll_jobs();
        let free = MAX_CONCURRENT_FETCHES.saturating_sub(self.jobs.len());
        for request in take_account_avatar_requests(free) {
            match spawn_fetch(request.clone(), Arc::clone(&self.fetcher)) {
                Some(job) => self.jobs.push(job),
                None => {
                    let _ = complete_account_avatar_request(&request, None);
                }
            }
            changed = true;
        }
        changed
    }

    fn poll_jobs(&mut self) -> bool {
        let mut changed = false;
        let mut index = 0;
        while index < self.jobs.len() {
            match self.jobs[index].rx.try_recv() {
                Ok(Ok(bytes)) => {
                    let job = self.jobs.swap_remove(index);
                    let _ = complete_account_avatar_request(&job.request, Some(bytes));
                    changed = true;
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    let job = self.jobs.swap_remove(index);
                    let _ = complete_account_avatar_request(&job.request, None);
                    changed = true;
                }
                Err(TryRecvError::Empty) => index += 1,
            }
        }
        changed
    }
}

fn spawn_fetch(request: AccountAvatarFetchRequest, fetcher: Fetcher) -> Option<FetchJob> {
    let (tx, rx) = mpsc::channel();
    let worker_request = request.clone();
    std::thread::Builder::new()
        .name("de-account-avatar".into())
        .spawn(move || {
            let _ = tx.send(fetcher(&worker_request));
        })
        .ok()?;
    Some(FetchJob { request, rx })
}

#[cfg(test)]
mod tests {
    use super::*;
    use de_editor_ui::account_avatar_runtime::{
        account_avatar_image, cached_account_avatar_bytes, register_account_avatar_url,
    };
    use std::time::{Duration, Instant};

    fn png_header() -> Vec<u8> {
        let mut bytes = vec![0; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&16_u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&16_u32.to_be_bytes());
        bytes
    }

    fn pump_until_idle(host: &mut AccountAvatarHost) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.is_pending() {
            host.pump();
            assert!(Instant::now() < deadline, "avatar worker never drained");
            std::thread::yield_now();
        }
    }

    #[test]
    fn scripted_success_and_failure_land_without_network() {
        let _guard = lock_avatar_test_registry();
        let url = "https://cdn.example/avatar-host-success.png";
        assert!(register_account_avatar_url(Some(url)));
        assert!(account_avatar_image().is_none());
        let mut host = AccountAvatarHost::with_fetcher(Arc::new(|_| Ok(png_header())));
        assert!(host.pump());
        pump_until_idle(&mut host);
        let image = account_avatar_image().expect("scripted image cached");
        assert!(cached_account_avatar_bytes(image.image_id).is_some());

        let failed_url = "https://cdn.example/avatar-host-failure.png";
        assert!(register_account_avatar_url(Some(failed_url)));
        assert!(account_avatar_image().is_none());
        let mut failed =
            AccountAvatarHost::with_fetcher(Arc::new(|_| Err(AvatarFetchError::RequestFailed)));
        failed.pump();
        pump_until_idle(&mut failed);
        assert!(account_avatar_image().is_none());
        assert!(take_account_avatar_requests(1).is_empty());
        assert!(register_account_avatar_url(None));
    }

    #[test]
    fn url_guards_reject_unsafe_targets() {
        for url in [
            "http://cdn.example/avatar.png",
            "https://user@cdn.example/avatar.png",
            "https://cdn.example/avatar.png#fragment",
            "https://cdn.example:0/avatar.png",
        ] {
            assert_eq!(
                fetch_account_avatar_blocking(url),
                Err(AvatarFetchError::UrlNotAllowed)
            );
        }
    }
}
