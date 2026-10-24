//! Editor v3 geometry (#86): a full-bleed canvas under floating cards.

use super::*;
use crate::widgets::host_canvas_geometry::{
    canvas_region, chrome_hit_at, layer_panel_rect, over_canvas, pinned_chat, property_panel_rect,
    toolbar_rect, PinnedChat,
};
use crate::widgets::ChromeHit;
use de_editor_core::{LeftPanelTab, NodeId};

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn state(open: bool) -> EditorState {
    let mut state = EditorState::new();
    state.editor_ui.sidebar_open = open;
    state
}

fn centre(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn the_canvas_is_the_whole_window_whatever_is_open() {
    let mut closed = state(false);
    let open = state(true);
    assert_eq!(canvas_region(&closed, W, H), (0.0, 0.0, W, H));
    assert_eq!(canvas_region(&open, W, H), (0.0, 0.0, W, H));
    // A selection shows the properties card without shrinking the canvas.
    closed.set_single_selection(NodeId::new("n1"));
    assert_eq!(canvas_region(&closed, W, H), (0.0, 0.0, W, H));
}

#[test]
fn the_left_card_floats_inset_and_slides_fully_out_when_shut() {
    let open = state(true);
    let card = layer_panel_rect(&open, H);
    assert_eq!(card.origin.x, CARD_INSET);
    assert_eq!(card.origin.y, LEFT_CARD_TOP);
    assert_eq!(card.origin.y + card.size.y, H - CARD_INSET);
    assert!(left_card_on_screen(&open));

    let shut = state(false);
    let card = layer_panel_rect(&shut, H);
    assert!(
        card.origin.x + card.size.x <= 0.0,
        "fully off screen: {card:?}"
    );
    assert!(!left_card_on_screen(&shut));
}

#[test]
fn the_toggle_and_rail_sit_beside_the_open_card_and_at_the_edge_when_shut() {
    let shut = state(false);
    let toggle = panel_toggle_rect(&shut);
    assert_eq!(toggle.origin.x, CARD_INSET);
    let rail = toolbar_rect(&shut, W, H, 200.0);
    assert_eq!(rail.origin.x, toggle.origin.x);
    assert!(rail.origin.y >= toggle.origin.y + toggle.size.y);

    let open = state(true);
    let card = layer_panel_rect(&open, H);
    let toggle = panel_toggle_rect(&open);
    assert_eq!(toggle.origin.x, card.origin.x + card.size.x + CARD_GAP);
    assert_eq!(toolbar_rect(&open, W, H, 200.0).origin.x, toggle.origin.x);
}

#[test]
fn the_toggle_is_chrome_and_the_cards_block_the_canvas() {
    let mut open = state(true);
    let toggle = panel_toggle_rect(&open);
    assert_eq!(
        chrome_hit_at(&open, W, H, centre(toggle)),
        Some(ChromeHit::TogglePanel)
    );
    let card = layer_panel_rect(&open, H);
    let p = centre(card);
    assert!(
        !over_canvas(&open, p.x, p.y, W, H),
        "no clicks through the card"
    );
    // Shut, the same spot is canvas again.
    open.editor_ui.sidebar_open = false;
    assert!(over_canvas(&open, p.x, p.y, W, H));

    // The Code view keeps the properties card up without a selection.
    open.editor_ui.property_tab = de_editor_core::PropertyTab::Code;
    assert!(open.right_rail_visible());
    let right = property_panel_rect(&open, W, H);
    assert_eq!(right.origin.x + right.size.x, W - CARD_INSET);
    let q = centre(right);
    assert!(!over_canvas(&open, q.x, q.y, W, H));
    // Below the properties card sits the zoom pill, not the card.
    assert!(right.origin.y + right.size.y < H - STATUS_BAR_HEIGHT - STATUS_INSET);
}

#[test]
fn the_composer_pill_is_centred_at_the_floor_while_the_panel_is_shut() {
    let shut = state(false);
    let Some(PinnedChat::Composer { x, bottom, width }) = pinned_chat(&shut, W, H) else {
        panic!("shut panel shows the composer pill");
    };
    assert!(
        ((x + width / 2.0) - W / 2.0).abs() < 1.0,
        "centred: x={x} w={width}"
    );
    assert!(bottom < H && bottom > H - 40.0);
}

#[test]
fn opening_runs_the_pill_into_the_card_and_then_pins_the_chat() {
    let mut ui_state = state(false);
    ui_state.editor_ui.slides_panel.tab = LeftPanelTab::Chat;
    let Some(PinnedChat::Composer { x: pill_x, .. }) = pinned_chat(&ui_state, W, H) else {
        panic!("pill");
    };
    ui_state.editor_ui.toggle_panel_animated(0);
    ui_state.editor_ui.tick_panel_motion(60);
    let reveal = panel_reveal(&ui_state);
    assert!(reveal > 0.0 && reveal < 1.0);
    let Some(PinnedChat::Composer { x: mid_x, .. }) = pinned_chat(&ui_state, W, H) else {
        panic!("still the travelling pill mid-motion");
    };
    assert!(mid_x < pill_x, "the pill moves left towards the card");
    // Paint and hit-test agree mid-motion: one reveal value.
    let card = layer_panel_rect(&ui_state, H);
    assert!(card.origin.x < CARD_INSET && card.origin.x + card.size.x > 0.0);

    ui_state
        .editor_ui
        .tick_panel_motion(de_editor_core::editor_ui_state::PANEL_MOTION_MS);
    assert!(matches!(
        pinned_chat(&ui_state, W, H),
        Some(PinnedChat::At(_))
    ));
}

#[test]
fn the_composer_pill_keeps_off_the_properties_card_on_a_narrow_window() {
    let (w, h) = (1024.0, 700.0);
    let mut s = state(true);
    s.editor_ui.slides_panel.tab = LeftPanelTab::Layers;
    s.editor_ui.property_tab = de_editor_core::PropertyTab::Code;
    let right = property_panel_rect(&s, w, h);
    match pinned_chat(&s, w, h) {
        Some(PinnedChat::Composer { x, width, .. }) => {
            assert!(
                x + width <= right.origin.x,
                "pill {x}+{width} runs under the card at {}",
                right.origin.x
            );
            let left = layer_panel_rect(&s, h);
            assert!(x >= left.origin.x + left.size.x);
        }
        // Too little room between the cards: no pill, never an overlap.
        Some(PinnedChat::Closed) => {}
        other => panic!("composer-only chat expected, got {other:?}"),
    }
}

#[test]
fn preview_hides_the_cards_and_hands_their_area_to_the_canvas() {
    let mut s = state(true);
    let p = centre(layer_panel_rect(&s, H));
    assert!(!over_canvas(&s, p.x, p.y, W, H));
    s.editor_ui.preview.mode = true;
    assert!(!left_card_on_screen(&s));
    assert!(over_canvas(&s, p.x, p.y, W, H));
    assert!(!toggle_panel(&mut s, 0), "the toggle is refused in Preview");
    assert!(s.editor_ui.sidebar_open);
}
