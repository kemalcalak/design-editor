//! Bounded host/widget handoff for the signed-in account's profile avatar.
//!
//! The URL arrives from the authenticated account profile and stays in this
//! ephemeral process-local slot; the encoded bytes stay beside it. The
//! desktop host owns SSRF-safe HTTPS and the existing image decode workers own
//! rasterization.

use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_AVATAR_ENCODED_BYTES: usize = 512 * 1024;
pub const MAX_AVATAR_SOURCE_EDGE_PX: u32 = 1_024;
pub const MAX_AVATAR_SOURCE_PIXELS: u64 = 1_048_576;

const MAX_AVATAR_URL_BYTES: usize = 2_048;
const MAX_ACCOUNT_REVISION_BYTES: usize = 128;
const ACCOUNT_AVATAR_MAX_RETRIES: u8 = 6;
const ACCOUNT_AVATAR_MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const IMAGE_ID_PREFIX: u64 = 0xa710_0000_0000_0000;

/// Opaque request token. Its custom `Debug` deliberately omits the URL.
#[derive(Clone)]
pub struct AccountAvatarFetchRequest {
    profile_generation: u64,
    image_id: u64,
    url: String,
}

impl AccountAvatarFetchRequest {
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl fmt::Debug for AccountAvatarFetchRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccountAvatarFetchRequest")
            .field("profile_generation", &self.profile_generation)
            .field("image_id", &self.image_id)
            .field("url", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct AccountAvatarImage {
    pub image_id: u64,
    pub encoded: Arc<[u8]>,
}

impl fmt::Debug for AccountAvatarImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccountAvatarImage")
            .field("image_id", &self.image_id)
            .field("encoded_bytes", &self.encoded.len())
            .finish()
    }
}

enum SlotState {
    Waiting,
    Queued,
    InFlight,
    Ready(Arc<[u8]>),
    Failed,
}

struct AvatarSlot {
    url: String,
    profile_generation: u64,
    image_id: u64,
    state: SlotState,
    failure_count: u8,
    retry_at: Option<Instant>,
}

impl AvatarSlot {
    fn image(&self) -> Option<AccountAvatarImage> {
        match &self.state {
            SlotState::Ready(encoded) => Some(AccountAvatarImage {
                image_id: self.image_id,
                encoded: Arc::clone(encoded),
            }),
            _ => None,
        }
    }

    fn request(&self) -> AccountAvatarFetchRequest {
        AccountAvatarFetchRequest {
            profile_generation: self.profile_generation,
            image_id: self.image_id,
            url: self.url.clone(),
        }
    }

    fn matches(&self, request: &AccountAvatarFetchRequest) -> bool {
        self.profile_generation == request.profile_generation
            && self.image_id == request.image_id
            && self.url == request.url
    }
}

struct AvatarRegistry {
    slot: Option<AvatarSlot>,
    next_image_id: u64,
}

impl AvatarRegistry {
    const fn new() -> Self {
        Self {
            slot: None,
            next_image_id: 1,
        }
    }

    fn fresh_image_id(&mut self) -> u64 {
        let image_id = IMAGE_ID_PREFIX | self.next_image_id;
        self.next_image_id = self.next_image_id.wrapping_add(1).max(1);
        image_id
    }

    /// Register `url`, queueing a fetch when the slot is waiting, and return
    /// the image when it is already ready.
    fn lookup(&mut self, url: &str) -> Option<AccountAvatarImage> {
        if !valid_url(url) {
            return None;
        }
        let mut profile_generation = 1;
        if let Some(slot) = self.slot.as_ref().filter(|slot| slot.url != url) {
            profile_generation = slot.profile_generation.wrapping_add(1).max(1);
            self.slot = None;
        }
        if self.slot.is_none() {
            let image_id = self.fresh_image_id();
            self.slot = Some(AvatarSlot {
                url: url.to_string(),
                profile_generation,
                image_id,
                state: SlotState::Waiting,
                failure_count: 0,
                retry_at: None,
            });
        }
        let slot = self.slot.as_mut().expect("slot was inserted above");
        if matches!(slot.state, SlotState::Waiting) {
            slot.state = SlotState::Queued;
        }
        slot.image()
    }

    fn ready(&mut self) -> Option<AccountAvatarImage> {
        let retry_url = self.slot.as_mut().and_then(|slot| {
            let retry_due = matches!(slot.state, SlotState::Failed)
                && slot
                    .retry_at
                    .is_some_and(|deadline| Instant::now() >= deadline);
            if matches!(slot.state, SlotState::Waiting) || retry_due {
                slot.state = SlotState::Waiting;
                slot.retry_at = None;
                Some(slot.url.clone())
            } else {
                None
            }
        });
        if let Some(url) = retry_url {
            let _ = self.lookup(&url);
        }
        self.slot.as_ref()?.image()
    }

    fn take_requests(&mut self, max: usize) -> Vec<AccountAvatarFetchRequest> {
        match self.slot.as_mut() {
            Some(slot) if max > 0 && matches!(slot.state, SlotState::Queued) => {
                slot.state = SlotState::InFlight;
                vec![slot.request()]
            }
            _ => Vec::new(),
        }
    }

    fn complete(&mut self, request: &AccountAvatarFetchRequest, bytes: Option<Vec<u8>>) -> bool {
        let Some(slot) = self
            .slot
            .as_mut()
            .filter(|slot| slot.matches(request) && matches!(slot.state, SlotState::InFlight))
        else {
            return false;
        };
        match bytes.and_then(validate_encoded_avatar) {
            Some(encoded) => {
                slot.failure_count = 0;
                slot.retry_at = None;
                slot.state = SlotState::Ready(encoded);
            }
            None => {
                slot.state = SlotState::Failed;
                if slot.failure_count < ACCOUNT_AVATAR_MAX_RETRIES {
                    slot.failure_count += 1;
                    slot.retry_at = Some(
                        Instant::now()
                            + account_avatar_retry_delay(slot.failure_count)
                                .min(ACCOUNT_AVATAR_MAX_RETRY_DELAY),
                    );
                } else {
                    slot.retry_at = None;
                }
            }
        }
        true
    }

    fn bytes_for(&self, image_id: u64) -> Option<Arc<[u8]>> {
        let slot = self
            .slot
            .as_ref()
            .filter(|slot| slot.image_id == image_id)?;
        slot.image().map(|image| image.encoded)
    }

    fn has_background_work(&self) -> bool {
        self.slot.as_ref().is_some_and(|slot| {
            matches!(
                slot.state,
                SlotState::Waiting | SlotState::Queued | SlotState::InFlight
            ) || (matches!(slot.state, SlotState::Failed) && slot.retry_at.is_some())
        })
    }

    fn install_ready(&mut self, source_identity: &str, bytes: Vec<u8>) -> bool {
        let Some(encoded) = validate_encoded_avatar(bytes) else {
            self.slot = None;
            return false;
        };
        if self
            .slot
            .as_ref()
            .is_some_and(|slot| slot.url != source_identity)
        {
            self.slot = None;
        }
        if self.slot.is_none() {
            let image_id = self.fresh_image_id();
            self.slot = Some(AvatarSlot {
                url: source_identity.to_string(),
                profile_generation: 1,
                image_id,
                state: SlotState::Waiting,
                failure_count: 0,
                retry_at: None,
            });
        }
        let slot = self.slot.as_mut().expect("ready slot was inserted above");
        slot.state = SlotState::Ready(encoded);
        slot.failure_count = 0;
        slot.retry_at = None;
        true
    }
}

static AVATAR: OnceLock<Mutex<AvatarRegistry>> = OnceLock::new();

fn avatar() -> &'static Mutex<AvatarRegistry> {
    AVATAR.get_or_init(|| Mutex::new(AvatarRegistry::new()))
}

/// Register or clear the current authenticated account's profile image.
pub fn register_account_avatar_url(url: Option<&str>) -> bool {
    let Ok(mut registry) = avatar().lock() else {
        return false;
    };
    let Some(url) = url else {
        registry.slot = None;
        return true;
    };
    if !valid_url(url) {
        registry.slot = None;
        return false;
    }
    let _ = registry.lookup(url);
    true
}

/// Resolve the current account image after its host fetch has completed.
pub fn account_avatar_image() -> Option<AccountAvatarImage> {
    avatar().lock().ok()?.ready()
}

/// Install bytes returned by the authenticated serve-web avatar proxy.
///
/// `revision` is an opaque, URL-derived identity emitted by the daemon. The
/// browser never receives the underlying profile URL.
pub fn install_account_avatar_bytes(revision: &str, bytes: Vec<u8>) -> bool {
    if revision.is_empty()
        || revision.len() > MAX_ACCOUNT_REVISION_BYTES
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return false;
    }
    let source_identity = format!("proxy:{revision}");
    avatar()
        .lock()
        .map(|mut registry| registry.install_ready(&source_identity, bytes))
        .unwrap_or(false)
}

/// Take the queued account fetch, if any, so a host worker can run it.
pub fn take_account_avatar_requests(max: usize) -> Vec<AccountAvatarFetchRequest> {
    avatar()
        .lock()
        .map(|mut registry| registry.take_requests(max))
        .unwrap_or_default()
}

/// Land one worker result if it still belongs to the registered profile.
/// Invalid, oversized, or stale results are discarded without retaining raw
/// bytes.
pub fn complete_account_avatar_request(
    request: &AccountAvatarFetchRequest,
    bytes: Option<Vec<u8>>,
) -> bool {
    avatar()
        .lock()
        .map(|mut registry| registry.complete(request, bytes))
        .unwrap_or(false)
}

pub fn cached_account_avatar_bytes(image_id: u64) -> Option<Arc<[u8]>> {
    avatar().lock().ok()?.bytes_for(image_id)
}

pub fn has_pending_account_avatar_requests() -> bool {
    avatar()
        .lock()
        .map(|registry| registry.has_background_work())
        .unwrap_or(false)
}

fn account_avatar_retry_delay(failure_count: u8) -> Duration {
    let exponent = u32::from(failure_count.saturating_sub(1).min(5));
    Duration::from_secs(1_u64 << exponent)
}

fn valid_url(url: &str) -> bool {
    !url.is_empty()
        && url.len() <= MAX_AVATAR_URL_BYTES
        && url.starts_with("https://")
        && !url.chars().any(char::is_whitespace)
}

fn validate_encoded_avatar(bytes: Vec<u8>) -> Option<Arc<[u8]>> {
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_ENCODED_BYTES {
        return None;
    }
    let (width, height) = crate::image_runtime::encoded_image_dimensions(&bytes)?;
    if width > MAX_AVATAR_SOURCE_EDGE_PX
        || height > MAX_AVATAR_SOURCE_EDGE_PX
        || u64::from(width) * u64::from(height) > MAX_AVATAR_SOURCE_PIXELS
    {
        return None;
    }
    Some(Arc::from(bytes.into_boxed_slice()))
}

#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn lock_account_avatar_registry_for_tests() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    if let Ok(mut registry) = avatar().lock() {
        *registry = AvatarRegistry::new();
    }
    guard
}

#[cfg(test)]
#[path = "account_avatar_runtime_tests.rs"]
mod tests;
