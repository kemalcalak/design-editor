//! Which browser nodes stream live (#324): the scheduling both hosts share,
//! next to the snapshot queue (`browser_render_queue`).
//!
//! Like pen.dev, a browser node plays live only while it is selected (or
//! interactive, phase 4); every other node keeps the snapshot queue's
//! still picture. A host calls [`BrowserStreamQueue::pump`] once per
//! frame, after the snapshot queue's pump, and carries out the returned
//! commands with its own transport (desktop: CDP screencasts; web: the
//! daemon's stream). Each landed frame goes into the node's live slot
//! (`browser_textures::set_live_frame`). On a [`BrowserStreamCommand::Stop`]
//! the host stops the stream and either keeps the last frame up
//! (`keep_frame`: the node was deselected, scrolled away or the window
//! went inactive; `browser_textures::end_live`, replaced by the next still)
//! or drops it (`browser_textures::forget_live`: the setting went off, the
//! pages are held, the document changed or the node lost its page).
//!
//! - Only selected (or interactive) nodes the last painted page frame
//!   showed stream, at most [`MAX_LIVE_PAGES`] of a multi-selection, the
//!   larger ones on screen first
//!   (`de_editor_core::browser_stream::pick_live_pages`).
//! - Nothing streams while the host is inactive (unfocused or hidden
//!   window), with the "Live pages" setting off, or while an opened
//!   document's pages are held.
//! - A quiet frame (same document revision, reloads, drained page frames,
//!   selection, interactive node and activity) walks nothing.
//! - Frame sizes follow the node's on-screen size in steps
//!   (`stream_frame_width`), so zooming re-encodes a stream only when it
//!   crosses a step.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use de_editor_core::browser_live::{browser_render_request, BrowserRenderRequest};
use de_editor_core::browser_stream::{
    pick_live_pages, stream_frame_width, stream_tier, BrowserStreamSpec, LiveCandidate, StreamHost,
    MAX_LIVE_PAGES,
};
use de_editor_core::{EditorState, NodeId};

use crate::widgets::browser_render_queue::BrowserRenderQueue;
use crate::widgets::browser_textures;

/// What a host does with one node's stream.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserStreamCommand {
    /// Start streaming the node's page.
    Start {
        node_id: String,
        spec: BrowserStreamSpec,
    },
    /// The running stream changes: `reload` when the page must load again
    /// (a new address or a Reload); otherwise only its layout or frame
    /// size moved.
    Update {
        node_id: String,
        spec: BrowserStreamSpec,
        reload: bool,
    },
    /// Stop the node's stream; `keep_frame`: its last frame stays up as the
    /// node's picture until the next still snapshot replaces it.
    Stop { node_id: String, keep_frame: bool },
}

/// Inputs of a scan; any change rescans.
#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    revision: u64,
    reload_generation: u64,
    visible_frames: u64,
    selection: u64,
    enabled: bool,
    dpi_bits: u32,
}

struct Running {
    spec: BrowserStreamSpec,
    reload: u32,
}

/// One live candidate with what its stream needs.
struct Wanted<'a> {
    candidate: LiveCandidate<'a>,
    request: BrowserRenderRequest,
    reload: u32,
    screen_width: f32,
}

pub struct BrowserStreamQueue {
    host: StreamHost,
    running: HashMap<String, Running>,
    last: Option<Stamp>,
    document_generation: u64,
}

impl BrowserStreamQueue {
    /// A queue for `host`'s streams (its caps, `stream_tier`).
    pub fn new(host: StreamHost) -> Self {
        Self {
            host,
            running: HashMap::new(),
            last: None,
            document_generation: 0,
        }
    }

    /// Nodes streaming now.
    pub fn running(&self) -> usize {
        self.running.len()
    }

    /// Whether `node_id` streams now.
    pub fn is_running(&self, node_id: &str) -> bool {
        self.running.contains_key(node_id)
    }

    /// Per-frame step. `snapshots` is the snapshot queue pumped this frame
    /// (its visible set), `dpi` the window's scale and `active` whether
    /// the host may stream at all (focused and shown).
    pub fn pump(
        &mut self,
        state: &EditorState,
        snapshots: &BrowserRenderQueue,
        dpi: f32,
        active: bool,
    ) -> Vec<BrowserStreamCommand> {
        let mut commands = Vec::new();
        if state.document_generation() != self.document_generation {
            // A new document: its node ids name other nodes.
            self.document_generation = state.document_generation();
            self.stop_all(&mut commands, false);
            self.last = None;
        }
        if snapshots.visible().is_empty() && self.running.is_empty() {
            // No browser on screen and nothing streaming: no scan needed
            // (and no selection hashing on documents without pages).
            self.last = None;
            return commands;
        }
        let live = &state.editor_ui.browser_live;
        let stamp = Stamp {
            revision: state.document_revision(),
            reload_generation: live.reload_generation(),
            visible_frames: snapshots.visible_frames(),
            selection: selection_hash(state),
            enabled: active && live.live_pages() && live.pages_allowed(),
            dpi_bits: dpi.to_bits(),
        };
        if self.last == Some(stamp) {
            return commands;
        }
        self.last = Some(stamp);
        if !stamp.enabled {
            // A window away keeps the last frames up; the setting off or
            // held pages show the stills.
            self.stop_all(&mut commands, !active);
            return commands;
        }
        self.scan(state, snapshots.visible(), dpi, &mut commands);
        commands
    }

    fn scan(
        &mut self,
        state: &EditorState,
        visible: &[String],
        dpi: f32,
        commands: &mut Vec<BrowserStreamCommand>,
    ) {
        let folder = state.editor_ui.document_folder.as_deref();
        let interactive = state.editor_ui.browser_live.interactive_node();
        let mut wanted: Vec<Wanted<'_>> = Vec::new();
        for id in visible {
            let node_id = NodeId::new(id);
            let selected = state.selection.set.contains(&node_id);
            let is_interactive = interactive == Some(id.as_str());
            // Unselected nodes keep their still: nothing to look up.
            if !selected && !is_interactive {
                continue;
            }
            let Some((request, reload)) = request_for(state, id, folder) else {
                continue;
            };
            let Some([width, height]) = browser_textures::visible_size(id) else {
                continue;
            };
            wanted.push(Wanted {
                candidate: LiveCandidate {
                    node_id: id.as_str(),
                    screen_area: width * height,
                    selected,
                    interactive: is_interactive,
                },
                request,
                reload,
                screen_width: width,
            });
        }
        let candidates: Vec<LiveCandidate<'_>> = wanted.iter().map(|w| w.candidate).collect();
        let picked = pick_live_pages(&candidates, MAX_LIVE_PAGES);
        let mut leaving: Vec<String> = self
            .running
            .keys()
            .filter(|id| !picked.contains(&id.as_str()))
            .cloned()
            .collect();
        leaving.sort_unstable();
        for node_id in leaving {
            self.running.remove(&node_id);
            // Deselected or scrolled away: the last frame stays. A node
            // whose page is gone shows what the snapshot queue shows.
            let keep_frame = request_for(state, &node_id, folder).is_some();
            commands.push(BrowserStreamCommand::Stop {
                node_id,
                keep_frame,
            });
        }
        let tier = stream_tier(self.host);
        for want in &wanted {
            let id = want.candidate.node_id;
            if !picked.contains(&id) {
                continue;
            }
            let current = self.running.get(id);
            let frame_width = stream_frame_width(
                current.map(|r| r.spec.frame_width),
                want.screen_width,
                dpi,
                tier.max_width,
            );
            let spec = BrowserStreamSpec::for_request(&want.request, frame_width, tier);
            let node_id = id.to_string();
            match current {
                None => commands.push(BrowserStreamCommand::Start {
                    node_id: node_id.clone(),
                    spec: spec.clone(),
                }),
                Some(running) if running.spec == spec && running.reload == want.reload => continue,
                Some(running) => commands.push(BrowserStreamCommand::Update {
                    node_id: node_id.clone(),
                    spec: spec.clone(),
                    reload: running.spec.url != spec.url || running.reload != want.reload,
                }),
            }
            self.running.insert(
                node_id,
                Running {
                    spec,
                    reload: want.reload,
                },
            );
        }
    }

    fn stop_all(&mut self, commands: &mut Vec<BrowserStreamCommand>, keep_frame: bool) {
        let mut ids: Vec<String> = self.running.drain().map(|(id, _)| id).collect();
        ids.sort_unstable();
        commands.extend(ids.into_iter().map(|node_id| BrowserStreamCommand::Stop {
            node_id,
            keep_frame,
        }));
    }
}

/// The node's snapshot request and reload count, `None` when it is gone or
/// has no page.
fn request_for(
    state: &EditorState,
    id: &str,
    folder: Option<&str>,
) -> Option<(BrowserRenderRequest, u32)> {
    let node = de_editor_core::walkers::find_node(state.active_children(), &NodeId::new(id))?;
    let reload = state.editor_ui.browser_live.reload_count(id);
    browser_render_request(node, reload, folder).map(|request| (request, reload))
}

/// Identity of the selection and the interactive node.
fn selection_hash(state: &EditorState) -> u64 {
    let mut hasher = DefaultHasher::new();
    for id in state.selection.set.iter() {
        id.as_str().hash(&mut hasher);
    }
    state
        .editor_ui
        .browser_live
        .interactive_node()
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "browser_stream_queue_tests.rs"]
mod tests;
