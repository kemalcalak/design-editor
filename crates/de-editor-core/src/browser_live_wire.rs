//! Wire shapes of the serve-web daemon's live browser channel (#324): the
//! web editor streams its live browser nodes over ONE connection, so they
//! do not use up the browser's per-origin connection limit.
//!
//! - `GET` [`BROWSER_LIVE_PATH`] opens a server-sent event stream. Every
//!   `data:` line is one JSON [`BrowserLiveEvent`]; the first is
//!   [`BrowserLiveEvent::Hello`] with the id of this client.
//! - `POST` [`BROWSER_LIVE_PATH`] with a [`BrowserLiveSet`] replaces that
//!   client's live set and answers a [`BrowserLiveSetReply`]. An empty set
//!   stops every stream; closing the event stream does too.
//!
//! A node streams only while it is selected (as on pen.dev), so the live
//! set is the client's selected browser nodes; a node left out of a set
//! (deselected) stops, and unselected nodes keep their still snapshots.
//! The daemon streams at most [`WEB_LIVE_MAX_STREAMS`] nodes per client,
//! the first ones of the set, at most [`WEB_LIVE_MAX_FPS`] frames a second
//! each, and sends only the newest frame of a node. Plain serde types, no
//! transport, so the daemon and the wasm client share them.

use crate::browser_live::BrowserSnapshotSpec;

/// The route of both the event stream (`GET`) and the set call (`POST`).
pub const BROWSER_LIVE_PATH: &str = "/api/browser/live";
/// Streams per web client; later entries of a set are dropped.
pub const WEB_LIVE_MAX_STREAMS: usize = 3;
/// Frame-rate cap per stream on the web (only selected nodes stream).
pub const WEB_LIVE_MAX_FPS: f64 = 10.0;

/// One live node of a set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLiveEntry {
    /// The client's key for the node; events name it.
    pub key: String,
    /// The page and its layout viewport, as for a snapshot. The scroll
    /// fields are not used: a live page streams from its top.
    pub page: BrowserSnapshotSpec,
    /// Largest frame wanted, in device px; frames are scaled down to fit.
    pub frame_width: u32,
    pub frame_height: u32,
}

/// The body of `POST` [`BROWSER_LIVE_PATH`]: a client's whole live set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLiveSet {
    /// The id from the client's [`BrowserLiveEvent::Hello`].
    pub client: String,
    pub nodes: Vec<BrowserLiveEntry>,
}

/// The answer to a [`BrowserLiveSet`].
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLiveSetReply {
    pub ok: bool,
    /// Keys streaming now.
    #[serde(default)]
    pub live: Vec<String>,
    /// Keys whose address this server may not load.
    #[serde(default)]
    pub blocked: Vec<String>,
    /// Keys whose entry was malformed or whose stream failed to start.
    #[serde(default)]
    pub failed: Vec<String>,
    /// Keys over [`WEB_LIVE_MAX_STREAMS`].
    #[serde(default)]
    pub dropped: Vec<String>,
    /// Why the whole call was refused (`ok` false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Why a node's stream ended or never started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserLiveEndReason {
    /// The entry was malformed (address, sizes).
    Invalid,
    /// The server has no browser installed.
    NoBrowser,
    /// The page failed to load or the stream broke.
    Failed,
}

/// One event of the stream.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BrowserLiveEvent {
    /// The first event: the id this client sends its sets with.
    Hello { client: String },
    /// The newest frame of a node.
    Frame {
        key: String,
        /// Counts up per stream; gaps are frames replaced before they
        /// were sent.
        seq: u64,
        /// The JPEG's size in px (0 when its header could not be read).
        width: u32,
        height: u32,
        jpeg_base64: String,
    },
    /// The node's address is not allowed on this server.
    Blocked { key: String },
    /// The node's stream ended or did not start; it stays on its last
    /// frame until the set changes it.
    Ended {
        key: String,
        reason: BrowserLiveEndReason,
        message: String,
    },
}

#[cfg(test)]
#[path = "browser_live_wire_tests.rs"]
mod tests;
