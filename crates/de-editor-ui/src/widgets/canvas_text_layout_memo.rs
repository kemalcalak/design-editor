//! Per-node memo of the canvas text line layout (#208).
//!
//! [`text_edit_layout`](super::canvas_text_edit::text_edit_layout) wraps
//! and measures a text node's content. The canvas painter ran it for every
//! visible text node on every frame, which was most of a frame on a large
//! pen.dev document (619 text nodes: ~57% of a 55 ms frame before the
//! kerned word caches of #197, which make each measure cheaper but still
//! leave the wrap walk per frame). The result depends only on the node's
//! text and text style, its box, and the fonts the backend measures with,
//! so it is kept per node id and reused while those are unchanged.
//!
//! Fonts are the one input the scene does not carry: a host opts in by
//! passing its font-registry generation to [`begin_frame`] each canvas
//! frame, and a new generation drops every entry. Without it (a host that
//! cannot tell when its fonts change, exports, thumbnails) the memo is
//! bypassed and the layout is computed as before.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::canvas_text_edit::{text_edit_layout, TextEditLayout};
use crate::layout_scene::{SceneNode, SceneTextAlign};
use crate::{Rect, RenderBackend};

/// Entries kept before the memo starts over (a page switch leaves the
/// previous page's nodes behind until then).
const CAPACITY: usize = 20_000;

/// Everything on the node `text_edit_layout` reads.
#[derive(Clone, PartialEq)]
struct LayoutKey {
    text: Option<String>,
    font_family: String,
    font_size: f32,
    font_weight: u16,
    line_height: f32,
    letter_spacing: f32,
    italic: bool,
    text_wrap: bool,
    bounds: Rect,
    text_align: SceneTextAlign,
}

impl LayoutKey {
    fn matches(&self, node: &SceneNode) -> bool {
        self.text.as_deref() == node.text.as_deref()
            && self.font_family == node.font_family
            && same_f32(self.font_size, node.font_size)
            && self.font_weight == node.font_weight
            && same_f32(self.line_height, node.line_height)
            && same_f32(self.letter_spacing, node.letter_spacing)
            && self.italic == node.italic
            && self.text_wrap == node.text_wrap
            && same_rect(self.bounds, node.bounds)
            && self.text_align == node.text_align
    }

    fn of(node: &SceneNode) -> Self {
        Self {
            text: node.text.clone(),
            font_family: node.font_family.clone(),
            font_size: node.font_size,
            font_weight: node.font_weight,
            line_height: node.line_height,
            letter_spacing: node.letter_spacing,
            italic: node.italic,
            text_wrap: node.text_wrap,
            bounds: node.bounds,
            text_align: node.text_align,
        }
    }
}

/// Bitwise equality, so a NaN field still matches itself.
fn same_f32(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    same_f32(a.origin.x, b.origin.x)
        && same_f32(a.origin.y, b.origin.y)
        && same_f32(a.size.x, b.size.x)
        && same_f32(a.size.y, b.size.y)
}

struct Memo {
    /// The host's font generation for this frame, `None` = bypass.
    generation: Option<u64>,
    /// The generation `entries` were measured under.
    entries_generation: Option<u64>,
    entries: HashMap<String, (LayoutKey, Rc<TextEditLayout>)>,
    /// Layouts computed inside opted-in frames (misses), for perf guards.
    computed: u64,
}

thread_local! {
    static MEMO: RefCell<Memo> = RefCell::new(Memo {
        generation: None,
        entries_generation: None,
        entries: HashMap::new(),
        computed: 0,
    });
}

/// Start a canvas frame measured with fonts at `generation` (`None`: the
/// host cannot tell when its fonts change, so nothing is memoized).
pub fn begin_frame(generation: Option<u64>) {
    MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        memo.generation = generation;
        if generation.is_some() && memo.entries_generation != generation {
            memo.entries.clear();
            memo.entries_generation = generation;
        }
    });
}

/// End a canvas frame: later paints (exports, thumbnails, preview) bypass
/// the memo.
pub fn end_frame() {
    MEMO.with(|memo| memo.borrow_mut().generation = None);
}

/// The font generation the last opted-in canvas frame measured with, so
/// chrome painted after the canvas (layer rows) can memoize measurements
/// too. `None` until a host opts in.
pub(crate) fn font_generation() -> Option<u64> {
    MEMO.with(|memo| memo.borrow().entries_generation)
}

/// How many layouts opted-in canvas frames on this thread had to compute
/// (memo misses). A steady frame adds none; perf guards read it.
pub fn computed_layouts() -> u64 {
    MEMO.with(|memo| memo.borrow().computed)
}

/// [`text_edit_layout`] for `node`, reused from an earlier frame when the
/// node's text inputs and the font generation are unchanged.
pub(crate) fn text_layout(backend: &mut dyn RenderBackend, node: &SceneNode) -> Rc<TextEditLayout> {
    let active = MEMO.with(|memo| memo.borrow().generation.is_some());
    if !active {
        return Rc::new(text_edit_layout(backend, node));
    }
    let hit = MEMO.with(|memo| {
        memo.borrow()
            .entries
            .get(node.id.as_str())
            .filter(|(key, _)| key.matches(node))
            .map(|(_, layout)| Rc::clone(layout))
    });
    if let Some(layout) = hit {
        return layout;
    }
    let layout = Rc::new(text_edit_layout(backend, node));
    MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        memo.computed += 1;
        if memo.entries.len() >= CAPACITY {
            memo.entries.clear();
        }
        memo.entries
            .insert(node.id.clone(), (LayoutKey::of(node), Rc::clone(&layout)));
    });
    layout
}

#[cfg(test)]
#[path = "canvas_text_layout_memo_tests.rs"]
mod tests;
