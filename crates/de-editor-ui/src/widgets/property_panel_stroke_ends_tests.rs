//! #389: the Stroke section's Start / End rows (shared popover + swap)
//! and the Miter field. Each control is found through the shared walkers,
//! proven reachable by the panel's hit-tests, then driven through the
//! shared dispatch / commit and read back from the snapshot.

use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_sections as sections;
use crate::widgets::property_panel_stroke::StrokeStyleRows;
use crate::widgets::property_panel_stroke_ends::{end_menu_rows, field_rect, END_CAPS, LABEL_W};
use crate::widgets::property_panel_test_support::{state_from, visible_for, CountingBackend};
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use de_editor_core::fills::{node_stroke_style, StrokeStyleEdit};
use de_editor_core::interaction_edit::InteractionMenuTarget;
use de_editor_core::{EditorState, NodeId, PropertyFocus};
use jian_ops_schema::style::{StrokeEndCap, StrokeJoin};

fn panel_rect() -> Rect {
    Rect::xywh(0.0, 0.0, 272.0, 1400.0)
}

fn state(node: &str) -> EditorState {
    let mut state = state_from(&format!(
        r##"{{ "version": "1.0.0", "children": [ {node} ]}}"##
    ));
    state.set_single_selection(NodeId::new("n"));
    state
}

const STROKE: &str = r##""thickness":2,"fill":[{"type":"solid","color":"#111111"}]"##;

fn line(extra: &str) -> EditorState {
    state(&format!(
        r##"{{"type":"line","id":"n","x":0,"y":0,"x2":100,"y2":0,"stroke":{{{STROKE}{extra}}}}}"##
    ))
}

fn path(d: &str) -> EditorState {
    state(&format!(
        r##"{{"type":"path","id":"n","x":0,"y":0,"width":100,"height":100,"d":"{d}",
              "stroke":{{{STROKE}}}}}"##
    ))
}

fn panel(state: &EditorState) -> PropertyPanel {
    PropertyPanel::for_selection(state).expect("panel")
}

fn action_rects(panel: &PropertyPanel) -> Vec<(PropertyPanelAction, Rect)> {
    sections::action_button_rects_with_fill_picker(
        panel_rect(),
        visible_for(panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    )
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

fn rect_of(panel: &PropertyPanel, action: &PropertyPanelAction) -> Option<Rect> {
    action_rects(panel)
        .into_iter()
        .find(|(a, _)| a == action)
        .map(|(_, r)| r)
}

fn reachable(state: &EditorState, action: &PropertyPanelAction) -> Rect {
    let panel = panel(state);
    let rect = rect_of(&panel, action).unwrap_or_else(|| panic!("walker emits {action:?}"));
    assert_eq!(
        panel.hit_test_action(panel_rect(), center(rect)).as_ref(),
        Some(action),
        "a press on {action:?} reaches it"
    );
    rect
}

fn dispatch(state: &mut EditorState, action: &PropertyPanelAction) {
    use crate::widgets::property_panel_dispatch as d;
    let before = state.snapshot_for_history();
    let mut image_adjustment_drag = None;
    let mut effect_radius_drag = None;
    let _ = d::apply_property_action(
        state,
        action,
        d::PropertyActionContext {
            now_ms: 0,
            resolved_sizing_fallback: None,
            image_adjustment_drag: &mut image_adjustment_drag,
            effect_radius_drag: &mut effect_radius_drag,
        },
    );
    if d::updates_document(action) && state.snapshot_for_history() != before {
        state.history_push_past(before);
    }
}

fn node(state: &EditorState) -> &jian_ops_schema::node::PenNode {
    de_editor_core::walkers::find_node(state.active_children(), &NodeId::new("n")).expect("node")
}

fn toggle(end: bool) -> PropertyPanelAction {
    PropertyPanelAction::ToggleInteractionMenu(InteractionMenuTarget::StrokeEnd { end })
}

#[test]
fn effective_ends_fall_back_to_the_cap() {
    // No cap on a line reads round; an unset end follows the cap.
    let summary = panel(&line("")).snapshot.stroke_style.expect("style");
    assert_eq!(
        (summary.start, summary.end),
        (StrokeEndCap::Round, StrokeEndCap::Round)
    );
    assert_eq!(summary.miter_limit, 4.0);
    assert!(summary.open);

    let summary = panel(&line(
        r#","cap":"square","strokeEnd":"triangle_arrow","miterLimit":9"#,
    ))
    .snapshot
    .stroke_style
    .expect("style");
    assert_eq!(summary.start, StrokeEndCap::Square);
    assert_eq!(summary.end, StrokeEndCap::TriangleArrow);
    assert_eq!(summary.miter_limit, 9.0);
}

#[test]
fn only_open_strokes_show_the_end_rows() {
    let open = panel(&path("M0 0 L100 100"));
    assert!(open.snapshot.stroke_style.as_ref().unwrap().open);
    assert!(visible_for(&open).stroke_style_rows.ends);
    assert!(rect_of(&open, &toggle(false)).is_some());

    let closed = panel(&path("M0 0 L100 0 L100 100 Z"));
    assert!(!closed.snapshot.stroke_style.as_ref().unwrap().open);
    assert!(!visible_for(&closed).stroke_style_rows.ends);
    assert!(rect_of(&closed, &toggle(false)).is_none());
    assert!(rect_of(&closed, &toggle(true)).is_none());

    let rect = panel(&state(&format!(
        r##"{{"type":"rectangle","id":"n","x":0,"y":0,"width":10,"height":10,"stroke":{{{STROKE}}}}}"##
    )));
    assert_eq!(
        visible_for(&rect).stroke_style_rows,
        StrokeStyleRows {
            styled: true,
            ends: false,
            miter: true,
        }
    );
}

#[test]
fn end_rows_are_reachable_and_line_up() {
    let state = line("");
    let start = reachable(&state, &toggle(false));
    let end = reachable(&state, &toggle(true));
    let swap = reachable(
        &state,
        &PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::SwapEnds),
    );
    assert_eq!(start.origin.x, end.origin.x);
    assert_eq!(start.size.x, end.size.x);
    assert!(end.origin.y > start.origin.y);
    assert!(swap.origin.x >= start.origin.x + start.size.x);
    assert_eq!(swap.origin.y, start.origin.y);
}

#[test]
fn the_popover_lists_every_end_anchors_to_its_trigger_and_closes_after_a_pick() {
    let mut state = line("");
    for end in [false, true] {
        let rows = end_menu_rows(&panel(&state).labels, end);
        assert_eq!(rows.len(), END_CAPS.len());
        for ((action, _), cap) in rows.iter().zip(END_CAPS) {
            let edit = if end {
                StrokeStyleEdit::End(cap)
            } else {
                StrokeStyleEdit::Start(cap)
            };
            assert_eq!(action, &PropertyPanelAction::SetStrokeStyle(edit));
        }
    }

    let trigger = reachable(&state, &toggle(true));
    dispatch(&mut state, &toggle(true));
    assert!(state.editor_ui.interaction_menu_open);
    let open = panel(&state);
    assert_eq!(
        open.interaction_menu_anchor_rect(panel_rect()),
        Some(trigger),
        "the popover drops from the End trigger"
    );
    // Find the Diamond row by walking the popover.
    let mut picked = None;
    let mut y = 0.0;
    while y < panel_rect().size.y {
        let point = Point2D::new(trigger.origin.x + 20.0, y);
        if let crate::widgets::InteractionMenuHit::Row(action) =
            open.interaction_menu_hit(panel_rect(), point)
        {
            if action
                == PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::End(StrokeEndCap::Diamond))
            {
                picked = Some(action);
                break;
            }
        }
        y += 2.0;
    }
    let picked = picked.expect("the Diamond row is hit-testable");
    dispatch(&mut state, &picked);
    assert!(
        !state.editor_ui.interaction_menu_open,
        "a pick closes the popover"
    );
    assert_eq!(
        node_stroke_style(node(&state)).unwrap().stroke_end,
        Some(StrokeEndCap::Diamond)
    );
    assert_eq!(
        panel(&state).snapshot.stroke_style.unwrap().end,
        StrokeEndCap::Diamond
    );
    assert!(state.undo());
    assert_eq!(node_stroke_style(node(&state)).unwrap().stroke_end, None);
}

#[test]
fn swap_exchanges_the_ends_as_one_undo_step() {
    let mut state = line(r#","strokeStart":"circle","strokeEnd":"line_arrow""#);
    dispatch(
        &mut state,
        &PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::SwapEnds),
    );
    let summary = panel(&state).snapshot.stroke_style.unwrap();
    assert_eq!(
        (summary.start, summary.end),
        (StrokeEndCap::LineArrow, StrokeEndCap::Circle)
    );
    assert!(state.undo());
    let summary = panel(&state).snapshot.stroke_style.unwrap();
    assert_eq!(
        (summary.start, summary.end),
        (StrokeEndCap::Circle, StrokeEndCap::LineArrow)
    );
}

fn miter_rect(panel: &PropertyPanel) -> Option<Rect> {
    sections::editable_input_rects(
        panel_rect(),
        visible_for(panel),
        &panel.snapshot.fills,
        &panel.snapshot.effects,
    )
    .into_iter()
    .find(|(focus, _)| *focus == PropertyFocus::StrokeMiterLimit)
    .map(|(_, r)| r)
}

#[test]
fn the_miter_field_shows_only_for_a_miter_join() {
    let state = line(r#","join":"miter""#);
    let p = panel(&state);
    let rect = miter_rect(&p).expect("miter field");
    assert_eq!(
        p.hit_test(panel_rect(), center(rect)),
        Some(PropertyFocus::StrokeMiterLimit)
    );
    // Under the End row, lined up with the dropdowns.
    let end = rect_of(&p, &toggle(true)).unwrap();
    assert!(rect.origin.y > end.origin.y);
    assert_eq!(rect.origin.x, end.origin.x);
    // A line's default join is round: no Miter field.
    assert!(miter_rect(&panel(&line(""))).is_none());
}

fn commit_miter(state: &mut EditorState, draft: &str) {
    let p = panel(state);
    state.ui.property_focus = Some(PropertyFocus::StrokeMiterLimit);
    assert_eq!(
        crate::widgets::property_panel_commit::property_focus_initial(
            PropertyFocus::StrokeMiterLimit,
            &p
        ),
        crate::util::format_panel_number(p.snapshot.stroke_style.as_ref().unwrap().miter_limit)
    );
    state.ui.property_input.set_text(draft);
    assert!(crate::widgets::property_panel_commit::commit_property_focus(state));
}

#[test]
fn the_miter_commit_clamps_and_undoes_in_one_step() {
    let mut state = line(r#","join":"miter""#);
    commit_miter(&mut state, "250");
    assert_eq!(
        node_stroke_style(node(&state)).unwrap().miter_limit,
        Some(100.0)
    );
    assert_eq!(
        panel(&state).snapshot.stroke_style.unwrap().miter_limit,
        100.0
    );
    assert!(state.undo());
    assert_eq!(node_stroke_style(node(&state)).unwrap().miter_limit, None);

    commit_miter(&mut state, "0.5");
    assert_eq!(
        node_stroke_style(node(&state)).unwrap().miter_limit,
        Some(1.0)
    );
    // The default is written as absent.
    commit_miter(&mut state, "4");
    assert_eq!(node_stroke_style(node(&state)).unwrap().miter_limit, None);
}

#[test]
fn paint_agrees_with_the_walkers() {
    // `PropertyPanel::paint` debug-asserts each section ends at the next
    // section's top; paint every row combination.
    for state in [
        line(r#","join":"miter""#),
        line(""),
        path("M0 0 L100 0 L100 100 Z"),
        state(r##"{"type":"rectangle","id":"n","x":0,"y":0,"width":10,"height":10}"##),
    ] {
        let p = panel(&state);
        let mut backend = CountingBackend::default();
        p.paint(
            &mut PaintCx {
                backend: &mut backend,
            },
            panel_rect(),
        );
    }
    // The end names and the swap glyph's rows paint on a line.
    let p = panel(&line(r#","strokeEnd":"diamond""#));
    let mut backend = CountingBackend::default();
    p.paint(
        &mut PaintCx {
            backend: &mut backend,
        },
        panel_rect(),
    );
    assert!(backend.texts.iter().any(|t| t == p.labels.stroke_start));
    assert!(backend.texts.iter().any(|t| t == "Diamond"));
}

/// Width of `text` at 12 px, the estimate the Motion sweep uses (wider
/// than the 11 px the rows paint at).
fn chrome_width(text: &str) -> f32 {
    text.chars()
        .map(|c| {
            let code = c as u32;
            let combining = matches!(
                code,
                0x0300..=0x036F
                    | 0x0E31
                    | 0x0E34..=0x0E3A
                    | 0x0E47..=0x0E4E
                    | 0x0900..=0x0903
                    | 0x093A..=0x094F
                    | 0x0951..=0x0957
                    | 0x0962..=0x0963
            );
            if combining {
                0.0
            } else if code < 0x0250 {
                0.55
            } else if code < 0x2000 {
                0.62
            } else {
                1.0
            }
        })
        .sum::<f32>()
        * 12.0
}

/// Every Start / End / Miter label and end name fits its control in
/// every locale at the panel's width.
#[test]
fn every_end_label_fits_in_every_locale() {
    let width = crate::widgets::property_panel::PROPERTY_PANEL_WIDTH;
    let row = Rect::xywh(
        crate::widgets::property_panel_inputs::PAD_X,
        0.0,
        width - crate::widgets::property_panel_inputs::PAD_X * 2.0,
        28.0,
    );
    // `fit_select_trigger_label`'s inset (padding + chevron).
    let name_room = field_rect(row).size.x - 34.0;
    let label_room = LABEL_W - 6.0;
    for locale in de_editor_core::Locale::ALL {
        let ui = de_editor_core::editor_ui_state::EditorUiState {
            locale,
            ..Default::default()
        };
        let labels = sections::PropertyLabels::for_editor_ui(&ui);
        let mut checks = vec![
            (labels.stroke_start, label_room),
            (labels.stroke_end, label_room),
            (labels.stroke_miter, label_room),
        ];
        checks.extend(
            labels
                .stroke_end_names
                .iter()
                .map(|name| (*name, name_room)),
        );
        for (label, room) in checks {
            assert!(
                chrome_width(label) <= room,
                "{locale:?}: {label:?} is {:.0} px, room {room:.0} px",
                chrome_width(label)
            );
        }
    }
}

#[test]
fn the_join_row_toggles_the_miter_field() {
    let mut state = line("");
    assert!(!visible_for(&panel(&state)).stroke_style_rows.miter);
    dispatch(
        &mut state,
        &PropertyPanelAction::SetStrokeStyle(StrokeStyleEdit::Join(StrokeJoin::Miter)),
    );
    assert!(visible_for(&panel(&state)).stroke_style_rows.miter);
}
