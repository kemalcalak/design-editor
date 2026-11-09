//! The pure half of the live channel: which entries of a set may stream
//! ([`select`]) and what turns the running streams into them ([`plan`]).

use std::collections::{BTreeMap, HashSet};

use de_browser_render::{StreamSettings, Viewport};
use de_editor_core::browser_live_wire::{BrowserLiveEntry, WEB_LIVE_MAX_FPS, WEB_LIVE_MAX_STREAMS};

/// Frames are never wider or taller than this, whatever a client asks.
const MAX_FRAME_WIDTH: u32 = 1024;
const MAX_FRAME_HEIGHT: u32 = 2048;
/// Longest node key a client may send.
const MAX_KEY_LEN: usize = 256;

/// One stream as the daemon runs it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LiveSpec {
    pub key: String,
    pub url: String,
    pub viewport: Viewport,
    pub settings: StreamSettings,
}

/// A set sorted into what streams and what does not.
#[derive(Debug, Default)]
pub(crate) struct Selection {
    /// At most [`WEB_LIVE_MAX_STREAMS`], in the set's order.
    pub wanted: Vec<LiveSpec>,
    /// Refused by the URL policy.
    pub blocked: Vec<String>,
    /// Malformed, with why.
    pub invalid: Vec<(String, String)>,
    /// Allowed but over the cap.
    pub dropped: Vec<String>,
}

/// Sort `entries`: checked like a snapshot request (address normalized,
/// then resolved against `document_folder`), screened by `permitted`, and
/// capped. A repeated key keeps its first entry; blocked and malformed
/// entries take no stream slot.
pub(crate) fn select(
    entries: Vec<BrowserLiveEntry>,
    document_folder: Option<&str>,
    permitted: impl Fn(&str) -> bool,
) -> Selection {
    let mut selection = Selection::default();
    let mut seen = HashSet::new();
    for entry in entries {
        if !seen.insert(entry.key.clone()) {
            continue;
        }
        let key = entry.key;
        if key.is_empty() || key.len() > MAX_KEY_LEN {
            let message = format!("key must be 1..={MAX_KEY_LEN} bytes");
            selection.invalid.push((key, message));
            continue;
        }
        if entry.frame_width == 0 || entry.frame_height == 0 {
            let message = "frameWidth and frameHeight must be at least 1".to_string();
            selection.invalid.push((key, message));
            continue;
        }
        let page = match crate::web_browser_snapshot::checked_spec(entry.page) {
            Ok(page) => page,
            Err(error) => {
                selection.invalid.push((key, error.to_string()));
                continue;
            }
        };
        let url =
            de_editor_core::browser_live::resolve_page_url(&page.url, document_folder).into_owned();
        if !permitted(&url) {
            selection.blocked.push(key);
            continue;
        }
        if selection.wanted.len() >= WEB_LIVE_MAX_STREAMS {
            selection.dropped.push(key);
            continue;
        }
        selection.wanted.push(LiveSpec {
            key,
            url,
            viewport: Viewport {
                width: page.layout_width,
                height: page.layout_height,
                scale: page.scale,
                mobile: page.mobile,
            },
            settings: StreamSettings {
                max_width: entry.frame_width.min(MAX_FRAME_WIDTH),
                max_height: entry.frame_height.min(MAX_FRAME_HEIGHT),
                max_fps: WEB_LIVE_MAX_FPS,
                ..StreamSettings::default()
            },
        });
    }
    selection
}

/// A running stream whose spec changed in place.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Update {
    pub spec: LiveSpec,
    /// The layout viewport changed (`set_viewport`).
    pub viewport: bool,
    /// The frame size changed (`set_settings`).
    pub settings: bool,
}

/// What turns the running streams into the wanted ones.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct LivePlan {
    /// Keys to stop: gone from the set, or whose address changed.
    pub stop: Vec<String>,
    pub update: Vec<Update>,
    /// New keys, and keys whose address changed (stopped first).
    pub open: Vec<LiveSpec>,
}

/// The plan from `current` (running specs by key) to `wanted`. A new
/// address is a new page, so it reopens rather than navigating the tab.
pub(crate) fn plan(current: &BTreeMap<String, LiveSpec>, wanted: &[LiveSpec]) -> LivePlan {
    let mut out = LivePlan::default();
    for key in current.keys() {
        if !wanted.iter().any(|spec| &spec.key == key) {
            out.stop.push(key.clone());
        }
    }
    for spec in wanted {
        match current.get(&spec.key) {
            None => out.open.push(spec.clone()),
            Some(running) if running.url != spec.url => {
                out.stop.push(spec.key.clone());
                out.open.push(spec.clone());
            }
            Some(running) if running != spec => out.update.push(Update {
                spec: spec.clone(),
                viewport: running.viewport != spec.viewport,
                settings: running.settings != spec.settings,
            }),
            Some(_) => {}
        }
    }
    out
}
