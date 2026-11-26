use super::*;
use crate::widgets::test_capture_backend::CaptureBackend;

fn lines_for(rect: Rect, zoom: f32, limit: Option<Rect>) -> usize {
    let mut backend = CaptureBackend::default();
    let mut cx = PaintCx {
        backend: &mut backend,
    };
    let viewport = DocViewport {
        pan_x: 0.0,
        pan_y: 0.0,
        zoom,
    };
    paint_pixel_grid(&mut cx, rect, &viewport, limit);
    backend.lines
}

#[test]
fn one_line_per_document_pixel_on_each_axis() {
    // 40x20 screen px at 400 %: x = 0, 4, …, 40 and y = 0, 4, …, 20.
    assert_eq!(
        lines_for(Rect::xywh(0.0, 0.0, 40.0, 20.0), 4.0, None),
        11 + 6
    );
}

#[test]
fn a_partial_repaint_only_draws_the_lines_it_crosses() {
    let full = Rect::xywh(0.0, 0.0, 400.0, 400.0);
    let strip = Rect::xywh(0.0, 0.0, 400.0, 8.0);
    // All 101 vertical lines (clipped to the strip) but only y = 0, 4, 8.
    assert_eq!(lines_for(full, 4.0, Some(strip)), 101 + 3);
    let outside = Rect::xywh(500.0, 500.0, 10.0, 10.0);
    assert_eq!(lines_for(full, 4.0, Some(outside)), 0);
}

#[test]
fn a_degenerate_zoom_paints_nothing() {
    let rect = Rect::xywh(0.0, 0.0, 40.0, 40.0);
    assert_eq!(lines_for(rect, 0.5, None), 0);
    assert_eq!(lines_for(rect, f32::NAN, None), 0);
}
