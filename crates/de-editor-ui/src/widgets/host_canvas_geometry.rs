//! The canvas region — the single source of truth for canvas-relative
//! coordinates — plus the hit-tests that must derive from it.
//!
//! Per `crates/CLAUDE.md`'s coordinate invariant, EVERY input path that
//! reasons about the canvas MUST derive its rects from
//! [`canvas_region`]; reusing `LAYER_PANEL_WIDTH` directly is the
//! documented bug (paint collapses `canvas_left` to 0 when the sidebar
//! is closed). Both hosts previously carried byte-identical copies of
//! this math in `widget_host/geometry.rs`, which is exactly the shape of
//! drift the invariant is meant to prevent — so it lives here and each
//! host keeps a thin forwarding method.

use de_editor_core::host_drag_state::{AnchorDragTarget, MarqueeDragState};
use de_editor_core::EditorState;

use crate::layout_scene::LayoutScene;
use crate::widgets::slides_panel_flow;
use crate::widgets::{STATUS_BAR_HEIGHT, STATUS_BAR_WIDTH, TOOLBAR_WIDTH, TOP_BAR_HEIGHT};
use crate::{Point2D, Rect};

/// Breathing room between the floating chrome and the canvas edges, so
/// the pills don't visually touch it (per the 2026-05-10 user note that
/// asked for a little vertical spacing under the chat pill).
pub const AICHAT_INSET_BOTTOM: f32 = 12.0;
pub const AICHAT_INSET_LEFT: f32 = 12.0;
pub const TOOLBAR_INSET_X: f32 = 12.0;
pub const TOOLBAR_INSET_Y: f32 = 12.0;
pub const STATUS_INSET: f32 = 16.0;
/// Width of the composer-only card docked at the canvas floor. Wide
/// enough for a sentence and the control row, narrow enough that it
/// never reads as a panel covering the design.
pub const COMPOSER_CARD_W: f32 = 420.0;
/// Below this the canvas cannot hold the card at all.
const AI_CHAT_MINIMIZED_MIN_WIDTH_REF: f32 = 240.0;

/// Placement of the minimized AI chat bar inside the canvas region
/// `(cx0, cy0, cw, ch)`.
///
/// **Minimizing changes the panel's height, never its width.** `width` is
/// the expanded panel's width, resolved by the host from the one source it
/// uses for the expanded rect too, and `panel_position` is the position a
/// drag left behind. Both are threaded through so the bar's left and right
/// edges land exactly where the expanded panel's did — the bar used to
/// carry a width constant of its own, which is why collapsing a resized
/// panel visibly jumped its edges inward.
///
/// Vertically the bar still hugs the canvas floor: a minimized panel is a
/// dock, not a floating window, so it drops the top half of the anchor and
/// the y of a dragged position. Returns `None` when the canvas cannot hold
/// the narrowest bar, matching the expanded panel's "too small to place"
/// contract.
pub fn minimized_chat_bar_rect(
    anchor: de_editor_core::ChatAnchor,
    width: f32,
    panel_position: Option<(f32, f32)>,
    cx0: f32,
    cy0: f32,
    cw: f32,
    ch: f32,
) -> Option<Rect> {
    use crate::widgets::{AI_CHAT_MINIMIZED_HEIGHT, AI_CHAT_MINIMIZED_MIN_WIDTH};
    use de_editor_core::ChatAnchor;

    let available_w = cw - AICHAT_INSET_LEFT - AICHAT_INSET_BOTTOM;
    let bar_w = width.min(available_w);
    if bar_w < AI_CHAT_MINIMIZED_MIN_WIDTH || ch <= AI_CHAT_MINIMIZED_HEIGHT + 16.0 {
        return None;
    }
    let x = match panel_position {
        // The expanded panel sits wherever the drag left it, so the bar does
        // too — anything else slides sideways on collapse.
        Some((x, _)) if bar_w >= width => x,
        _ => match anchor {
            ChatAnchor::TopLeft | ChatAnchor::BottomLeft => cx0 + AICHAT_INSET_LEFT,
            ChatAnchor::TopRight | ChatAnchor::BottomRight => {
                cx0 + cw - bar_w - AICHAT_INSET_BOTTOM
            }
        },
    };
    Some(Rect {
        origin: Point2D::new(x, cy0 + ch - AI_CHAT_MINIMIZED_HEIGHT - AICHAT_INSET_BOTTOM),
        size: Point2D::new(bar_w, AI_CHAT_MINIMIZED_HEIGHT),
    })
}

/// Whether the generation workspace currently owns the layout: its
/// docked chat column IS the left panel (the rail's own tab row is not
/// shown while the workspace header/toolbar replace the TopBar), and
/// the dock's open/width read the shared `sidebar_open` /
/// `layer_panel_width`.
pub fn workspace_docked(state: &EditorState) -> bool {
    state.editor_ui.workspace.visible
}

/// Where a column-pinned chat panel sits, when it is pinned at all.
///
/// ONE answer for the two columns that can own the chat — the
/// workspace's dock and the professional editor's Chat tab — resolved
/// here so `ai_chat_rect` on either host is a routing decision, never
/// a second copy of the geometry. That second copy is exactly how the
/// two left columns drifted apart before they were folded into one
/// panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinnedChat {
    /// Pinned into the column at this rect.
    At(Rect),
    /// Pinned into a column that is currently closed — no chat at all.
    /// Distinct from "not pinned" so a pinned chat never falls back to
    /// its floating placement just because the panel was shut.
    Closed,
    /// The rail is showing another tab: only the composer appears, as a
    /// card docked at the canvas floor. Its height is the caller's to
    /// resolve (it depends on draft length, chips and focus), so this
    /// carries the card's left / bottom / width and the caller finishes
    /// it.
    Composer { x: f32, bottom: f32, width: f32 },
}

/// The pinned chat's placement, or `None` when the chat is not
/// column-pinned (it then keeps its ordinary floating geometry).
///
/// The workspace branch ignores the rail's tab: while the workspace is
/// up the dock IS the chat's home. The professional branch asks the
/// flow whether the Chat tab owns the rail, and the rect is the rail's
/// body below the tab row — the same subtraction the Layers tree's
/// rows start from.
///
/// When ANOTHER tab owns the rail the chat is simply not pinned, and
/// keeps its ordinary floating placement: the Chat tab is a new HOME
/// for the conversation, not a retirement of the floating panel, so
/// exactly one of the two shows at a time and nothing a user relies on
/// (drag, minimize, anchor) disappears with this change. Once the Chat
/// tab IS the home, though, shutting the rail or starting a
/// presentation means no chat at all — never a panel drifting back
/// over the canvas the user just cleared.
pub fn pinned_chat(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Option<PinnedChat> {
    let ui = &state.editor_ui;
    if ui.workspace.visible {
        return if ui.sidebar_open {
            Some(PinnedChat::At(Rect::xywh(
                0.0,
                de_editor_core::WORKSPACE_HEADER_H,
                ui.layer_panel_width,
                (viewport_h - de_editor_core::WORKSPACE_HEADER_H).max(0.0),
            )))
        } else {
            Some(PinnedChat::Closed)
        };
    }
    if ui.chat_composer_only() {
        let (cx0, cy0, cw, ch) = canvas_region(state, viewport_w, viewport_h);
        let _ = cy0;
        let width = COMPOSER_CARD_W.min((cw - AICHAT_INSET_LEFT * 2.0).max(0.0));
        if width < AI_CHAT_MINIMIZED_MIN_WIDTH_REF {
            return Some(PinnedChat::Closed);
        }
        return Some(PinnedChat::Composer {
            x: cx0 + AICHAT_INSET_LEFT,
            bottom: cy0 + ch - AICHAT_INSET_BOTTOM,
            width,
        });
    }
    if !slides_panel_flow::chat_tab_active(state) {
        return None;
    }
    if ui.preview.mode || !ui.sidebar_open {
        return Some(PinnedChat::Closed);
    }
    let panel = layer_panel_rect(state, viewport_h);
    Some(PinnedChat::At(slides_panel_flow::layers_content_rect(
        state, panel,
    )))
}

/// Top-left of the canvas region in viewport-logical px. Collapses to
/// `x = 0` when the sidebar is closed — the whole point of the
/// invariant. The top edge sits below the TopBar.
///
/// While the workspace is docked, the canvas starts at the dock edge —
/// which is the LEFT PANEL's edge (`layer_panel_width`, 0 when the
/// panel is closed) — below the workspace header + toolbar.
pub fn canvas_origin(state: &EditorState) -> (f32, f32) {
    if workspace_docked(state) {
        let dock_x = if state.editor_ui.sidebar_open {
            state.editor_ui.layer_panel_width
        } else {
            0.0
        };
        return (
            dock_x,
            de_editor_core::WORKSPACE_HEADER_H + de_editor_core::WORKSPACE_TOOLBAR_H,
        );
    }
    let cx0 = if state.editor_ui.sidebar_open {
        state.editor_ui.layer_panel_width
    } else {
        0.0
    };
    (cx0, TOP_BAR_HEIGHT)
}

/// Canvas region `(x, y, w, h)` in viewport-logical px. The right rail
/// only reserves width while it is actually visible, and both extents
/// clamp at zero so a viewport narrower than the rails can't produce a
/// negative-size rect. While the workspace is docked there is no right
/// rail, and the presentation family's deck strip reserves the bottom.
pub fn canvas_region(
    state: &EditorState,
    viewport_w: f32,
    viewport_h: f32,
) -> (f32, f32, f32, f32) {
    let (canvas_left, canvas_top) = canvas_origin(state);
    if workspace_docked(state) {
        let workspace = &state.editor_ui.workspace;
        let strip_h = if crate::widgets::workspace_surface::family_has_strip(workspace.family) {
            de_editor_core::WORKSPACE_DECK_STRIP_H
        } else {
            0.0
        };
        let canvas_w = (viewport_w - canvas_left).max(0.0);
        let canvas_h = (viewport_h - canvas_top - strip_h).max(0.0);
        return (canvas_left, canvas_top, canvas_w, canvas_h);
    }
    let canvas_right = if state.right_rail_visible() {
        viewport_w - state.editor_ui.property_panel_width
    } else {
        viewport_w
    };
    let canvas_w = (canvas_right - canvas_left).max(0.0);
    let canvas_h = (viewport_h - canvas_top).max(0.0);
    (canvas_left, canvas_top, canvas_w, canvas_h)
}

/// [`canvas_region`] as a `Rect` — the form the `CanvasViewport` paint
/// and the `AlignToolbar` placement both want.
pub fn canvas_rect(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Rect {
    let (x, y, w, h) = canvas_region(state, viewport_w, viewport_h);
    Rect {
        origin: Point2D::new(x, y),
        size: Point2D::new(w, h),
    }
}

/// Left rail. Callers gate on `editor_ui.sidebar_open` themselves — this
/// returns the rect the panel WOULD occupy.
pub fn layer_panel_rect(state: &EditorState, viewport_h: f32) -> Rect {
    Rect {
        origin: Point2D::new(0.0, TOP_BAR_HEIGHT),
        size: Point2D::new(
            state.editor_ui.layer_panel_width,
            (viewport_h - TOP_BAR_HEIGHT).max(0.0),
        ),
    }
}

/// Right rail. Callers gate on the panel actually resolving for the
/// current selection — this returns the rect it WOULD occupy.
pub fn property_panel_rect(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Rect {
    let width = state.editor_ui.property_panel_width;
    Rect {
        origin: Point2D::new(viewport_w - width, TOP_BAR_HEIGHT),
        size: Point2D::new(width, (viewport_h - TOP_BAR_HEIGHT).max(0.0)),
    }
}

/// Floating vertical toolbar, inset from the canvas top-left. `toolbar_h`
/// comes from the widget's own layout pass (it depends on how many tool
/// slots are live), which is why it is a parameter rather than derived.
pub fn toolbar_rect(state: &EditorState, toolbar_h: f32) -> Rect {
    let (canvas_left, canvas_top) = canvas_origin(state);
    Rect {
        origin: Point2D::new(canvas_left + TOOLBAR_INSET_X, canvas_top + TOOLBAR_INSET_Y),
        size: Point2D::new(TOOLBAR_WIDTH, toolbar_h),
    }
}

/// Whether the canvas is wide enough to float the toolbar without it
/// covering the whole band. Below this the hosts skip painting it.
pub fn toolbar_fits(canvas_w: f32) -> bool {
    canvas_w > TOOLBAR_WIDTH + TOOLBAR_INSET_X * 2.0
}

/// Intrinsic height of the floating tool column — the widget's own
/// layout pass, which depends on how many tool slots are live. Both
/// hosts used to re-spell this `LayoutCx` dance at every hit-test site.
pub fn toolbar_layout_height(state: &EditorState) -> f32 {
    use crate::widgets::{LayoutCx, Toolbar, Widget};
    Toolbar::for_editor(state)
        .layout(&LayoutCx {
            available_width: TOOLBAR_WIDTH,
            dpi: 1.0,
        })
        .rect
        .size
        .y
}

/// [`toolbar_rect`] measured against the live [`toolbar_layout_height`]
/// — the form every hit-test / hover site wants (paint already has the
/// height in hand from its own layout pass).
pub fn toolbar_rect_for(state: &EditorState) -> Rect {
    toolbar_rect(state, toolbar_layout_height(state))
}

/// Bottom-right floating StatusBar pill. `None` when the canvas is too
/// narrow to float it — the hosts' paint guard and their event-time
/// hit-test must agree, so both read this one answer.
pub fn status_bar_rect(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Option<Rect> {
    let (canvas_left, canvas_top, canvas_w, canvas_h) =
        canvas_region(state, viewport_w, viewport_h);
    if canvas_w <= STATUS_BAR_WIDTH + STATUS_INSET * 2.0 {
        return None;
    }
    let canvas_right = canvas_left + canvas_w;
    Some(Rect {
        origin: Point2D::new(
            canvas_right - STATUS_BAR_WIDTH - STATUS_INSET,
            canvas_top + canvas_h - STATUS_BAR_HEIGHT - STATUS_INSET,
        ),
        size: Point2D::new(STATUS_BAR_WIDTH, STATUS_BAR_HEIGHT),
    })
}

/// Screen-space rect of a live marquee drag, normalized so either drag
/// direction yields a positive-size rect. `None` below 1 px on either
/// axis — a sub-pixel band would paint as a stray hairline.
pub fn marquee_rect(m: &MarqueeDragState) -> Option<Rect> {
    let w = (m.current_screen_x - m.start_screen_x).abs();
    let h = (m.current_screen_y - m.start_screen_y).abs();
    if w < 1.0 || h < 1.0 {
        return None;
    }
    Some(Rect {
        origin: Point2D::new(
            m.start_screen_x.min(m.current_screen_x),
            m.start_screen_y.min(m.current_screen_y),
        ),
        size: Point2D::new(w, h),
    })
}

/// Whether `(x, y)` is inside the canvas region (inclusive of its
/// edges, matching the paint clip).
pub fn over_canvas(state: &EditorState, x: f32, y: f32, viewport_w: f32, viewport_h: f32) -> bool {
    let (cx0, cy0, cw, ch) = canvas_region(state, viewport_w, viewport_h);
    x >= cx0 && x <= cx0 + cw && y >= cy0 && y <= cy0 + ch
}

/// Screen → document point through the canvas region, WITHOUT the
/// region bound-check.
///
/// Every path that has already established it owns the gesture — a pen
/// session in flight, an anchor / handle drag, a shape-create drag, a
/// press that passed its own `over_canvas` guard — needs the document
/// point even when the cursor has wandered off the region, so it must
/// not go through [`canvas_doc_point`]'s `None`. Both hosts used to
/// re-spell the `(x - cx0, y - cy0)` offset inline at ~10 sites, which
/// is exactly the drift the coordinate invariant forbids.
pub fn canvas_doc_point_unclamped(state: &EditorState, x: f32, y: f32) -> Point2D {
    let (cx0, cy0) = canvas_origin(state);
    state.viewport.to_document(Point2D::new(x - cx0, y - cy0))
}

/// Screen → document point through the canvas region. `None` when the
/// point is outside the region.
pub fn canvas_doc_point(
    state: &EditorState,
    x: f32,
    y: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<Point2D> {
    over_canvas(state, x, y, viewport_w, viewport_h)
        .then(|| canvas_doc_point_unclamped(state, x, y))
}

/// Document point at the centre of the canvas region — where "insert
/// this here" actions (Component-Browser insert, clipboard paste,
/// dropped file) place their content.
pub fn canvas_centre_doc_point(state: &EditorState, viewport_w: f32, viewport_h: f32) -> Point2D {
    let (_cx0, _cy0, cw, ch) = canvas_region(state, viewport_w, viewport_h);
    state.viewport.to_document(Point2D::new(cw / 2.0, ch / 2.0))
}

/// 8 screen-px grab radius (TS `PATH_CONTROL_HIT_RADIUS`), squared and
/// expressed in doc space so the comparison stays multiplication-only.
fn grab_radius_sq(zoom: f32) -> f32 {
    64.0 / (zoom * zoom)
}

/// Hit-test the selected Path node's anchors / bezier handles at screen
/// point `(x, y)`. Returns `(node id, anchor index, what was grabbed)`.
///
/// Handles hit-test before anchors — TS `hitTestPathControl` walks
/// handleOut → handleIn across all anchors first
/// (`skia-hit-handlers.ts:136-159`). Ghost (unset) handles are grabbable
/// only with the Pen tool; the Select-tool editor shows existing handles
/// only (TS overlay parity).
pub fn path_anchor_hit(
    state: &EditorState,
    scene: &LayoutScene,
    x: f32,
    y: f32,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<(String, usize, AnchorDragTarget)> {
    use crate::layout_scene::NodeKind;
    use crate::widgets::path_handle_positions;
    use de_editor_core::pen::PathHandleSide;

    if !matches!(
        state.tool,
        de_editor_core::Tool::Pen | de_editor_core::Tool::Select
    ) {
        return None;
    }
    if state.selection_count() != 1 {
        return None;
    }
    let sel = state.selection.anchor.as_str().to_string();
    let node = scene.active_page()?.find(&sel)?;
    if !matches!(node.kind, NodeKind::Path) {
        return None;
    }
    let (cx0, cy0, _cw, _ch) = canvas_region(state, viewport_w, viewport_h);
    let zoom = state.viewport.zoom.max(0.0001);
    let canvas_local = Point2D::new(x - cx0, y - cy0);
    let mut doc = state.viewport.to_document(canvas_local);
    // Un-rotate the cursor into the node's local frame — handle
    // positions are stored unrotated but the path paints rotated.
    if node.rotation.abs() > f32::EPSILON {
        let b: Rect = node.aggregate_bounds();
        let centre = Point2D::new(b.origin.x + b.size.x / 2.0, b.origin.y + b.size.y / 2.0);
        doc = crate::widgets::rotate_point(doc, centre, -node.rotation);
    }
    let r2 = grab_radius_sq(zoom);
    let hit = |p: Point2D| (doc.x - p.x).powi(2) + (doc.y - p.y).powi(2) <= r2;
    let pen_tool = matches!(state.tool, de_editor_core::Tool::Pen);
    for (i, a) in node.path_anchors.iter().enumerate() {
        let (hin, hout) = path_handle_positions(a, zoom);
        if (a.handle_out.is_some() || pen_tool) && hit(hout) {
            return Some((
                sel.clone(),
                i,
                AnchorDragTarget::Handle(PathHandleSide::Out),
            ));
        }
        if (a.handle_in.is_some() || pen_tool) && hit(hin) {
            return Some((sel.clone(), i, AnchorDragTarget::Handle(PathHandleSide::In)));
        }
    }
    for (i, a) in node.path_anchors.iter().enumerate() {
        if hit(a.pos) {
            return Some((sel.clone(), i, AnchorDragTarget::Anchor));
        }
    }
    // Paths without resolved anchor data fall back to `points`.
    if node.path_anchors.is_empty() {
        for (i, p) in node.points.iter().enumerate() {
            if hit(*p) {
                return Some((sel.clone(), i, AnchorDragTarget::Anchor));
            }
        }
    }
    None
}
