//! Vector edit mode (#387) through the host: enter / leave, point
//! selection and drags with one undo step, Delete heal, pen continue,
//! Shift constraint, snapping, marquee, and a pixel comparison of an
//! imported icon before and after its first edit.

use super::WidgetHostNative;
use de_editor_core::vector_edit::PointRef;
use de_editor_core::{NodeId, Tool};
use jian_ops_schema::node::PenNode;

const VW: f32 = 1440.0;
const VH: f32 = 900.0;

fn host_with(json: &str) -> WidgetHostNative {
    let mut host = WidgetHostNative::new();
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = de_editor_core::EditorState::from_document(doc);
    host.editor_state_mut().editor_ui.sidebar_open = false;
    host.mark_paint_dirty_for_test();
    host
}

/// A 3-anchor open path plus a rectangle to snap to.
fn path_host() -> WidgetHostNative {
    host_with(
        r#"{"version":"1.0.0","children":[
          {"type":"path","id":"p","name":"p","x":300,"y":300,"width":80,"height":60,
           "anchors":[{"x":300,"y":300},{"x":380,"y":300},{"x":380,"y":360}]},
          {"type":"rectangle","id":"r","name":"r","x":500,"y":500,"width":40,"height":40}
        ]}"#,
    )
}

fn screen(host: &WidgetHostNative, x: f32, y: f32) -> (f32, f32) {
    let (cx0, cy0, _, _) = host.canvas_region(VW, VH);
    let vp = &host.editor_state().viewport;
    (cx0 + vp.pan_x + x * vp.zoom, cy0 + vp.pan_y + y * vp.zoom)
}

fn anchors(host: &WidgetHostNative) -> Vec<(f64, f64)> {
    match de_editor_core::walkers::find_node(
        host.editor_state().active_children(),
        &NodeId::new("p"),
    ) {
        Some(PenNode::Path(p)) => p
            .anchors
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|a| (a.x, a.y))
            .collect(),
        _ => panic!("path missing"),
    }
}

fn enter(host: &mut WidgetHostNative) {
    host.editor_state_mut()
        .set_single_selection(NodeId::new("p"));
    assert!(host.apply_send(), "Enter enters vector edit mode");
    assert!(host.vector_edit_active());
}

fn drag(host: &mut WidgetHostNative, from: (f32, f32), to: (f32, f32)) {
    let a = screen(host, from.0, from.1);
    let b = screen(host, to.0, to.1);
    host.apply_press(a.0, a.1, VW, VH);
    host.apply_cursor_move((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    host.apply_cursor_move(b.0, b.1);
    host.apply_release_with_viewport(VW, VH);
}

#[test]
fn enter_and_escape_leave_the_document_alone() {
    let mut host = path_host();
    let before = anchors(&host);
    enter(&mut host);
    assert!(host.apply_escape());
    assert!(!host.vector_edit_active());
    assert_eq!(anchors(&host), before);
}

#[test]
fn point_drag_moves_one_point_in_one_undo_step() {
    let mut host = path_host();
    enter(&mut host);
    drag(&mut host, (380.0, 300.0), (400.0, 320.0));
    let a = anchors(&host);
    assert_eq!(a[0], (300.0, 300.0));
    assert!(
        (a[1].0 - 400.0).abs() < 0.01 && (a[1].1 - 320.0).abs() < 0.01,
        "{a:?}"
    );
    host.apply_undo();
    assert_eq!(anchors(&host)[1], (380.0, 300.0));
}

#[test]
fn shift_drag_constrains_to_45_degrees() {
    let mut host = path_host();
    enter(&mut host);
    host.set_modifier_shift(true);
    drag(&mut host, (380.0, 300.0), (430.0, 304.0));
    host.set_modifier_shift(false);
    let a = anchors(&host);
    assert!((a[1].1 - 300.0).abs() < 1e-6, "kept horizontal: {a:?}");
}

#[test]
fn drag_snaps_to_another_node_corner_and_cmd_suspends() {
    let mut host = path_host();
    enter(&mut host);
    drag(&mut host, (380.0, 360.0), (502.0, 497.0));
    assert_eq!(anchors(&host)[2], (500.0, 500.0));
    // Near the opposite corner (540, 540): Cmd held, no snap.
    host.set_modifier_cmd(true);
    drag(&mut host, (500.0, 500.0), (541.5, 538.5));
    host.set_modifier_cmd(false);
    let a = anchors(&host)[2];
    assert!(
        (a.0 - 541.5).abs() < 0.01 && (a.1 - 538.5).abs() < 0.01,
        "{a:?}"
    );
}

#[test]
fn delete_removes_the_selected_point() {
    let mut host = path_host();
    enter(&mut host);
    let (x, y) = screen(&host, 380.0, 300.0);
    host.apply_press(x, y, VW, VH);
    host.apply_release_with_viewport(VW, VH);
    assert!(host.apply_delete());
    assert_eq!(anchors(&host), vec![(300.0, 300.0), (380.0, 360.0)]);
    assert!(
        host.vector_edit_active(),
        "the path survives with two points"
    );
}

#[test]
fn marquee_selects_points_and_arrows_nudge_them() {
    let mut host = path_host();
    enter(&mut host);
    drag(&mut host, (370.0, 290.0), (390.0, 370.0));
    let selected = host.editor_state().vector_edit().unwrap().selected.clone();
    assert_eq!(selected, [PointRef::new(0, 1), PointRef::new(0, 2)].into());
    assert!(host.apply_nudge(10.0, 0.0));
    assert_eq!(anchors(&host)[1], (390.0, 300.0));
    assert_eq!(anchors(&host)[0], (300.0, 300.0));
}

#[test]
fn pen_continues_the_selected_path_from_its_end() {
    let mut host = path_host();
    host.editor_state_mut()
        .set_single_selection(NodeId::new("p"));
    host.apply_set_tool(Tool::Pen);
    let (x, y) = screen(&host, 380.0, 360.0);
    assert!(host.apply_press(x, y, VW, VH));
    host.apply_release_with_viewport(VW, VH);
    assert!(host.vector_edit_active());
    let (x, y) = screen(&host, 450.0, 362.0);
    host.set_modifier_shift(true);
    host.apply_press(x, y, VW, VH);
    host.apply_release_with_viewport(VW, VH);
    host.set_modifier_shift(false);
    let a = anchors(&host);
    assert_eq!(a.len(), 4);
    assert!(
        (a[3].1 - 360.0).abs() < 1e-6,
        "Shift keeps the new segment level: {a:?}"
    );
}

#[test]
fn double_click_on_a_path_enters_the_mode() {
    // A filled triangle, so a press inside it hits the path.
    let mut host = host_with(
        r##"{"version":"1.0.0","children":[
          {"type":"path","id":"p","name":"p","x":300,"y":300,"width":80,"height":60,
           "closed":true,"fill":[{"type":"solid","color":"#000000"}],
           "anchors":[{"x":300,"y":300},{"x":380,"y":300},{"x":380,"y":360}]}
        ]}"##,
    );
    let (x, y) = screen(&host, 365.0, 320.0);
    host.apply_press(x, y, VW, VH);
    host.apply_release_with_viewport(VW, VH);
    host.apply_press(x, y, VW, VH);
    host.apply_release_with_viewport(VW, VH);
    assert!(host.vector_edit_active());
}

/// Rasterize `d` fitted into `rect` the way the canvas does.
fn raster(d: &str, rect: de_editor_ui::Rect) -> Vec<u8> {
    let path = skia_safe::utils::parse_path::from_svg(d).expect("d parses");
    let mut path = crate::backend::skia::fit_path_to_rect(&path, rect);
    path.set_fill_type(skia_safe::PathFillType::EvenOdd);
    let mut surface = skia_safe::surfaces::raster_n32_premul((96, 96)).expect("surface");
    let canvas = surface.canvas();
    canvas.clear(skia_safe::Color::WHITE);
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(skia_safe::Color::BLACK);
    canvas.draw_path(&path, &paint);
    let info = surface.image_info();
    let mut px = vec![0u8; info.compute_min_byte_size()];
    let row = info.min_row_bytes();
    assert!(surface.read_pixels(&info, &mut px, row, (0, 0)));
    px
}

#[test]
fn imported_icon_edits_without_visual_change() {
    // Arcs + a compound hole, 24 px native drawn at 64 px.
    const D: &str = "M12 2a10 10 0 1 0 0 20a10 10 0 1 0 0-20zM12 7a5 5 0 1 1 0 10a5 5 0 0 1 0-10z";
    let mut host = host_with(&format!(
        r#"{{"version":"1.0.0","children":[
          {{"type":"path","id":"p","name":"icon","x":16,"y":16,"width":64,"height":64,
            "fillRule":"evenodd","d":"{D}"}}]}}"#
    ));
    let before = raster(D, de_editor_ui::Rect::xywh(16.0, 16.0, 64.0, 64.0));
    enter(&mut host);
    host.editor_state_mut()
        .vector_select(PointRef::new(0, 0), false);
    // Two opposite nudges: a real write-back that ends where it began.
    assert!(host.apply_nudge(1.0, 0.0));
    assert!(host.apply_nudge(-1.0, 0.0));
    let Some(PenNode::Path(p)) = de_editor_core::walkers::find_node(
        host.editor_state().active_children(),
        &NodeId::new("p"),
    ) else {
        panic!("path missing")
    };
    let baked = p.d.clone().expect("d");
    assert_ne!(baked, D, "the first edit re-bakes d");
    let rect = de_editor_ui::Rect::xywh(
        p.base.x.unwrap() as f32,
        p.base.y.unwrap() as f32,
        de_editor_core::pen_node_ext::sizing_px(&p.width).unwrap() as f32,
        de_editor_core::pen_node_ext::sizing_px(&p.height).unwrap() as f32,
    );
    let after = raster(&baked, rect);
    let differing = before
        .chunks(4)
        .zip(after.chunks(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 24))
        .count();
    let worst = before
        .iter()
        .zip(after.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    // Skia flattens cubics more coarsely than the conics it builds for
    // `A` commands, so edge pixels shift by a fraction of a pixel's
    // coverage; the shape itself is within 0.25 px (see the core arc
    // tests). Anything larger — a moved, rescaled or refilled shape —
    // changes far more pixels, and fully.
    assert!(
        differing * 100 <= 96 * 96 && worst < 128,
        "{differing} pixels changed (worst {worst}) after the round trip"
    );
}
