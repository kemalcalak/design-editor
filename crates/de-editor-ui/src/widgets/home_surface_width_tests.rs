//! Home's labelled controls are hit-tested at the size they are painted
//! at, in every locale (#440): the kind chips, the size chip, the model
//! slot, the language button, Open / New file, the import cards and the
//! MCP connect popover.

use super::copy::{kind_label, size_chip_label, tr};
use super::layout::locale_code;
use super::HomeSurface;
use crate::widgets::label_fit_test_support::{assert_inside, fresh_backend, run_in};
use crate::widgets::label_widths::LabelWidths;
use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};
use de_editor_core::home_mcp_setup::home_mcp_agent_name;
use de_editor_core::{EditorState, HomeFamily, HomeHit, Locale};

const W: f32 = 1440.0;
const H: f32 = 900.0;

fn home_state(locale: Locale) -> EditorState {
    let mut state = EditorState::default();
    state.editor_ui.locale = locale;
    state.editor_ui.home.show(false);
    state
}

fn paint(home: &HomeSurface<'_>, backend: &mut FamilyGapBackend) {
    home.paint(&mut PaintCx { backend }, Rect::xywh(0.0, 0.0, W, H));
}

/// The point just inside a box's right edge, mid height.
fn right_edge(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x - 0.5,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn the_page_controls_agree_in_every_locale() {
    for locale in Locale::ALL {
        for usable in [false, true] {
            let state = home_state(locale);
            let mut home = HomeSurface::for_editor(&state).unwrap();
            if usable {
                home.usable_agent = true;
                home.chip_label = "sonnet · Claude".to_string();
            }
            let mut backend = fresh_backend();
            paint(&home, &mut backend);
            let layout = home.layout(W, H);
            assert_eq!(
                home.measured_layout(W, H, &mut FamilyGapBackend::uniform()),
                layout,
                "{locale:?}: a repaint measures nothing new"
            );
            let what = format!("{locale:?} usable={usable}");

            for (rect, family) in layout.chips.iter().copied().zip(HomeFamily::ALL) {
                assert_inside(
                    run_in(&backend, kind_label(locale, family), rect, &what),
                    rect,
                    12.0,
                    &what,
                );
                assert_eq!(
                    home.hit_test(W, H, right_edge(rect)),
                    Some(HomeHit::Kind(family))
                );
            }
            let size = home.state.task_draft().size;
            let size_label = size_chip_label(locale, size);
            assert_inside(
                run_in(&backend, &size_label, layout.size_chip, &what),
                layout.size_chip,
                8.0,
                &what,
            );
            assert_eq!(
                home.hit_test(W, H, right_edge(layout.size_chip)),
                Some(HomeHit::SizeChip)
            );

            let model = layout.model_chip;
            if usable {
                let label = run_in(&backend, &home.chip_label, model, &what);
                // The label ends where the chevron's clip begins.
                assert_inside(
                    label,
                    Rect::xywh(model.origin.x, 0.0, model.size.x - 20.0, 1.0),
                    0.0,
                    &what,
                );
                assert_eq!(
                    home.hit_test(W, H, right_edge(model)),
                    Some(HomeHit::ModelChip)
                );
            } else {
                let label = run_in(&backend, tr(locale, "home.model.addProvider"), model, &what);
                // Clear of the trailing arrow (18 px from the right edge).
                assert_inside(
                    label,
                    Rect::xywh(model.origin.x, 0.0, model.size.x - 18.0, 1.0),
                    4.0,
                    &what,
                );
                assert_eq!(
                    home.hit_test(W, H, right_edge(model)),
                    Some(HomeHit::AddProvider)
                );
            }

            let code = locale_code(locale);
            assert_inside(
                run_in(&backend, &code, layout.locale_button, &what),
                layout.locale_button,
                10.0,
                &what,
            );
            assert_eq!(
                home.hit_test(W, H, right_edge(layout.locale_button)),
                Some(HomeHit::Locale)
            );
            let open = run_in(
                &backend,
                tr(locale, "home.top.openFile"),
                layout.open_file,
                &what,
            );
            assert_inside(open, layout.open_file, 12.0, &what);
            assert_eq!(
                home.hit_test(W, H, right_edge(layout.open_file)),
                Some(HomeHit::OpenFile)
            );
            let new = run_in(
                &backend,
                tr(locale, "home.top.newFile"),
                layout.new_file,
                &what,
            );
            assert_inside(new, layout.new_file, 12.0, &what);
            assert_eq!(
                home.hit_test(W, H, right_edge(layout.new_file)),
                Some(HomeHit::NewFile)
            );
        }
    }
}

/// A chip is exactly as wide as its semibold label plus the icon and
/// padding, under a face wider than the old estimate.
#[test]
fn a_chip_grows_with_its_painted_label() {
    let state = home_state(Locale::EnUs);
    let home = HomeSurface::for_editor(&state).unwrap();
    let mut backend = fresh_backend();
    let measured = home.measured_layout(W, H, &mut backend);
    let label = crate::widgets::text_metrics::measure_in_family_weighted(
        &mut backend,
        kind_label(Locale::EnUs, HomeFamily::ALL[0]),
        super::layout::CHIP_FONT,
        super::SANS,
        600,
    );
    assert_eq!(measured.chips[0].size.x, 12.0 + 16.0 + 8.0 + label + 12.0);
    crate::widgets::label_widths::reset();
    let narrow = home.measured_layout(W, H, &mut FamilyGapBackend::uniform());
    assert!(measured.chips[0].size.x > narrow.chips[0].size.x);
}

#[test]
fn the_mcp_popover_agrees_in_every_locale() {
    for locale in Locale::ALL {
        let mut state = home_state(locale);
        state.editor_ui.home.mcp_popover_open = true;
        let home = HomeSurface::for_editor(&state).unwrap();
        assert!(!home.mcp.running, "the Start button is under test");
        let mut backend = fresh_backend();
        paint(&home, &mut backend);
        let layout = home.layout(W, H);
        let popover = home.mcp_popover(&layout, W);
        assert_eq!(
            home.measured_mcp_popover(&layout, W, &mut FamilyGapBackend::uniform()),
            popover,
            "{locale:?}"
        );
        let what = format!("{locale:?}");
        let hit = |point| home.hit_test(W, H, point);

        let start = run_in(&backend, tr(locale, "home.mcp.start"), popover.start, &what);
        assert_inside(start, popover.start, 10.0, &what);
        assert_eq!(hit(right_edge(popover.start)), Some(HomeHit::McpStart));
        for (cli, rect) in &popover.tabs {
            assert_inside(
                run_in(&backend, home_mcp_agent_name(*cli), *rect, &what),
                *rect,
                9.0,
                &what,
            );
            assert_eq!(hit(right_edge(*rect)), Some(HomeHit::McpAgent(*cli)));
        }
        let action = run_in(
            &backend,
            tr(locale, "home.mcp.setUp"),
            popover.action,
            &what,
        );
        assert_inside(action, popover.action, 12.0, &what);
        assert_eq!(hit(right_edge(popover.action)), Some(popover.action_hit));
        let link = run_in(
            &backend,
            tr(locale, "home.mcp.settings"),
            popover.settings_link,
            &what,
        );
        let link_box = popover.settings_link;
        assert_inside(
            link,
            Rect::xywh(link_box.origin.x, 0.0, link_box.size.x - 12.0, 1.0),
            0.0,
            &what,
        );
        assert_eq!(hit(right_edge(link_box)), Some(HomeHit::McpSettings));
        for (rect, target) in [
            (popover.copy_endpoint, HomeHit::McpCopyEndpoint),
            (popover.copy_snippet, HomeHit::McpCopySnippet),
        ] {
            let copy = backend
                .runs
                .iter()
                .find(|r| {
                    r.text == tr(locale, "home.mcp.copy")
                        && r.origin.x > rect.origin.x
                        && r.origin.x < rect.origin.x + rect.size.x
                        && r.origin.y > rect.origin.y
                        && r.origin.y < rect.origin.y + rect.size.y
                })
                .unwrap_or_else(|| panic!("{what}: copy label"));
            assert_inside(copy, rect, 8.0, &what);
            assert_eq!(hit(right_edge(rect)), Some(target));
        }
        for line in &popover.intro_lines {
            let width = LabelWidths::cached().styled(line, 12.0, super::SANS, 400);
            assert!(width <= popover.header.size.x + 0.01, "{what}: {line:?}");
        }
    }
}

/// The IME caret sits where the composer paints its caret: after the text
/// before it measured in the input's own face (jian's `TextArea` draws in
/// Inter), on the caret's wrapped line.
#[test]
fn the_ime_caret_follows_the_painted_text() {
    let mut state = home_state(Locale::EnUs);
    state.editor_ui.home.input.set_text("Design a pricing page");
    state.editor_ui.home.input.set_caret("Design".len(), 0);
    let home = HomeSurface::for_editor(&state).unwrap();
    let layout = home.layout(W, H);
    let mut backend = fresh_backend();
    let caret = home.focused_input_caret_rect(W, H, &mut backend);
    let before =
        crate::widgets::text_metrics::measure_in_family(&mut backend, "Design", 15.0, "Inter");
    assert_eq!(caret.origin.x, layout.input_box.origin.x + 20.0 + before);
    let first_row = caret.origin.y;

    // A text long enough to wrap: the caret at its end is on a lower row.
    let long = "word ".repeat(80);
    state.editor_ui.home.input.set_text(long.trim_end());
    let home = HomeSurface::for_editor(&state).unwrap();
    let wrapped = home.focused_input_caret_rect(W, H, &mut backend);
    assert!(wrapped.origin.y > first_row, "{wrapped:?}");
    assert!(wrapped.origin.x <= layout.input_box.origin.x + layout.input_box.size.x - 20.0 + 0.01);
}
