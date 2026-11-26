//! "Send to agent" on a sticky note / context / prompt annotation (#398,
//! annotations from #12): open the chat with the note's text as the draft
//! and the note plus the layers around it staged as `@`-mentions, so the
//! agent reads what the note is about without the user retyping it.
//!
//! The draft is not sent: the user reads it, edits it and presses Send.

use jian_ops_schema::node::PenNode;

use crate::chat_mentions::{ChatMention, MentionKind};
use crate::find_layers::node_text_content;
use crate::pen_node_ext::PenNodeExt;
use crate::{EditorState, NodeId};

/// How far past the note's edges a layer may start and still count as
/// "around" it, in document px.
pub const SURROUNDING_MARGIN: f64 = 24.0;
/// With nothing overlapping, the nearest layer whose centre is within this
/// distance of the note's centre is taken instead.
pub const NEAREST_RADIUS: f64 = 600.0;
/// Layers staged next to the note at most.
pub const MAX_SURROUNDING: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Bounds {
    fn of(node: &PenNode) -> Option<Self> {
        let base = node.base();
        Some(Self {
            x: base.x.unwrap_or(0.0),
            y: base.y.unwrap_or(0.0),
            w: node.width_px()?,
            h: node.height_px()?,
        })
    }

    fn grown(self, by: f64) -> Self {
        Self {
            x: self.x - by,
            y: self.y - by,
            w: self.w + by * 2.0,
            h: self.h + by * 2.0,
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    fn centre_distance(self, other: Self) -> f64 {
        let dx = (self.x + self.w / 2.0) - (other.x + other.w / 2.0);
        let dy = (self.y + self.h / 2.0) - (other.y + other.h / 2.0);
        (dx * dx + dy * dy).sqrt()
    }
}

fn label_of(node: &PenNode) -> String {
    node.base()
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| node.type_tag())
        .to_string()
}

fn node_mention(node: &PenNode) -> ChatMention {
    ChatMention {
        kind: MentionKind::Node,
        key: node.id_str().to_string(),
        label: label_of(node),
    }
}

/// The top-level layers of the active page around `note`: those reaching
/// within [`SURROUNDING_MARGIN`] of it (in page order), else the nearest one
/// within [`NEAREST_RADIUS`]; other annotations never count. At most
/// [`MAX_SURROUNDING`].
pub fn surrounding_layers<'a>(page: &'a [PenNode], note: &PenNode) -> Vec<&'a PenNode> {
    let Some(note_box) = Bounds::of(note) else {
        return Vec::new();
    };
    let candidates = page
        .iter()
        .filter(|node| node.id_str() != note.id_str() && node.annotation().is_none())
        .filter_map(|node| Bounds::of(node).map(|b| (node, b)));
    let area = note_box.grown(SURROUNDING_MARGIN);
    let touching: Vec<&PenNode> = candidates
        .clone()
        .filter(|(_, b)| area.intersects(*b))
        .map(|(node, _)| node)
        .take(MAX_SURROUNDING)
        .collect();
    if !touching.is_empty() {
        return touching;
    }
    candidates
        .map(|(node, b)| (node, note_box.centre_distance(b)))
        .filter(|(_, d)| *d <= NEAREST_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(node, _)| vec![node])
        .unwrap_or_default()
}

impl EditorState {
    /// Whether `id` is an annotation on the active page (the layer menu
    /// offers "Send to agent" only then).
    pub fn is_annotation(&self, id: &NodeId) -> bool {
        crate::walkers::find_node(self.active_children(), id)
            .is_some_and(|node| node.annotation().is_some())
    }

    /// Open the chat on annotation `id`: a fresh tab when the current one
    /// already holds a conversation or a running turn, the note's text as
    /// the draft, and the note plus its surrounding layers as staged
    /// `@`-mentions. False when `id` is not an annotation on the active
    /// page or the editor has no chat to open.
    pub fn send_annotation_to_agent(&mut self, id: &NodeId, now_ms: u64) -> bool {
        let Some(note) = crate::walkers::find_node(self.active_children(), id) else {
            return false;
        };
        if note.annotation().is_none() {
            return false;
        }
        let text = node_text_content(note).unwrap_or_default();
        let mut mentions = vec![node_mention(note)];
        for node in surrounding_layers(self.active_children(), note) {
            let mention = node_mention(node);
            if !mentions.contains(&mention) {
                mentions.push(mention);
            }
        }
        // The conversation lives in the rail's Agent tab; a host without
        // it keeps the floating panel, which `expand` below opens.
        self.editor_ui.enter_chat_tab();
        if !self.chat.messages.is_empty() || self.chat.has_streaming_turn() {
            self.chat.new_tab();
        }
        let tokens: Vec<String> = mentions.iter().map(ChatMention::token).collect();
        let draft = match text.trim() {
            "" => tokens.join(" "),
            body => format!("{body}\n\n{}", tokens.join(" ")),
        };
        self.chat.input.set_text(draft);
        self.chat.pending_mentions = mentions;
        self.chat.expand();
        self.chat.focus_input_at_end(now_ms);
        true
    }
}

#[cfg(test)]
#[path = "annotation_agent_tests.rs"]
mod tests;
