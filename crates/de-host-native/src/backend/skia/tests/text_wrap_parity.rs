//! Wrapping text paints as many lines as layout sized its box for (#186):
//! the canvas painter breaks lines at the node's exact width
//! (`canvas_text_edit::text_edit_layout`), and `fit_content` heights come
//! from the layout measure (`de_pen_loader::layout_measure_backend`) at that
//! same width.

use super::*;
use de_editor_ui::layout_scene::{NodeKind, SceneNode};
use de_editor_ui::widgets::canvas_text_edit::text_edit_layout;

const FAMILY: &str = "Inter";
const SIZE: f32 = 16.0;
const TEXT: &str =
    "Design tools should break lines where the browser does, at the exact box width.";

fn wrapping_node(text: &str, width: f32, weight: u16) -> SceneNode {
    let mut node = SceneNode::leaf("t", NodeKind::Text);
    node.bounds = Rect::xywh(0.0, 0.0, width, 400.0);
    node.text = Some(text.to_string());
    node.font_family = FAMILY.to_string();
    node.font_size = SIZE;
    node.font_weight = weight;
    node.text_wrap = true;
    node
}

fn layout_line_count(text: &str, width: f32, weight: u16) -> u16 {
    let run = StyledRun {
        text,
        font_family: Some(FAMILY),
        font_size: SIZE,
        font_weight: weight,
        font_style: FontStyleKind::Normal,
        letter_spacing: 0.0,
    };
    de_pen_loader::layout_measure_backend()
        .measure(&MeasureRequest {
            runs: &[run],
            line_height: 0.0,
            max_width: Some(width),
        })
        .line_count
}

#[cfg_attr(
    target_os = "windows",
    ignore = "WINDOWS_SKIA_DIRECTWRITE_TEXT_MEASURE_ABORT: SkiaMeasure paragraph layout aborts in Windows CI; macOS and Linux keep coverage"
)]
#[test]
fn painted_wrap_matches_the_layout_line_count_at_every_width() {
    let _guard = crate::font_registry_test_support::lock();
    super::super::font_fallback_tests::register_test_bundled_fonts();
    let mut native = NativeBackend::with_dpi(1.0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((1, 1)).expect("surface");
    let mut frame = crate::NativeFrameBackend::new(&mut native, surface.canvas());

    // 400 is a real face; 600 is painted as synthetic bold over it.
    for weight in [400, 600] {
        let mut width = 90.0;
        while width < 640.0 {
            let painted = text_edit_layout(&mut frame, &wrapping_node(TEXT, width, weight));
            assert_eq!(
                painted.lines.len(),
                usize::from(layout_line_count(TEXT, width, weight)),
                "weight {weight}: painted lines {:?} at width {width} disagree with layout",
                painted.lines
            );
            width += 3.0;
        }
    }
}

#[cfg_attr(
    target_os = "windows",
    ignore = "WINDOWS_SKIA_DIRECTWRITE_TEXT_MEASURE_ABORT: SkiaMeasure paragraph layout aborts in Windows CI; macOS and Linux keep coverage"
)]
#[test]
fn a_line_one_px_wider_than_the_box_wraps() {
    let _guard = crate::font_registry_test_support::lock();
    super::super::font_fallback_tests::register_test_bundled_fonts();
    let mut native = NativeBackend::with_dpi(1.0);
    let line = "Exact width wrapping";
    let natural = native.measure_text_family_styled(line, SIZE, FAMILY, 400, false);
    let mut surface = skia_safe::surfaces::raster_n32_premul((1, 1)).expect("surface");
    let mut frame = crate::NativeFrameBackend::new(&mut native, surface.canvas());

    let fits = text_edit_layout(&mut frame, &wrapping_node(line, natural.ceil(), 400));
    assert_eq!(fits.lines, vec![line]);
    // One px short of the line: the old slack (min(5%, fontSize / 2) = 8 px
    // here) painted it on one line while layout sized the box for two.
    let short = text_edit_layout(&mut frame, &wrapping_node(line, natural.ceil() - 1.0, 400));
    assert_eq!(short.lines.len(), 2, "lines: {:?}", short.lines);
    assert_eq!(layout_line_count(line, natural.ceil() - 1.0, 400), 2);
}
