//! The label-sized chrome #451 moved off its own heuristics: the Prompt
//! Center and Asset Center chips, the chat's chip row, footer, header and
//! minimized bar, and the design.md body, in all 15 locales. Each label is
//! checked against the box the backend-less hit-test resolves right after
//! paint, as well as for family and weight (`assert_drawn_as_measured`).

use de_editor_core::{AssetCenterTab, EditorState};
use de_i18n::Locale;

use super::assert_drawn_as_measured;
use crate::widgets::label_fit_test_support::assert_inside;
use crate::widgets::test_family_gap_backend::{FamilyGapBackend, PaintedRun};
use crate::widgets::Widget;
use crate::Rect;

/// jian's `TextInputView` / `TextArea` / `Button` paint in their own face.
const JIAN_COMPONENT_FAMILY: &str = "Inter";

/// The run whose pen starts inside `rect` (its baseline within the box's
/// height, its left edge within its width), other than the `skip` glyphs.
fn run_starting_in<'a>(
    backend: &'a FamilyGapBackend,
    rect: Rect,
    skip: &[&str],
    what: &str,
) -> &'a PaintedRun {
    backend
        .runs
        .iter()
        .find(|run| {
            !skip.contains(&run.text.as_str())
                && run.origin.y > rect.origin.y
                && run.origin.y <= rect.origin.y + rect.size.y
                && run.origin.x >= rect.origin.x - 0.01
                && run.origin.x < rect.origin.x + rect.size.x
        })
        .unwrap_or_else(|| {
            let painted: Vec<_> = backend
                .runs
                .iter()
                .map(|run| (&run.text, run.origin.x, run.origin.y))
                .collect();
            panic!("{what}: no label starts in {rect:?}; painted {painted:?}")
        })
}

/// `rect` less `left` and `right` px.
fn inner(rect: Rect, left: f32, right: f32) -> Rect {
    Rect::xywh(
        rect.origin.x + left,
        rect.origin.y,
        (rect.size.x - left - right).max(0.0),
        rect.size.y,
    )
}

#[test]
fn prompt_center_chips_hold_their_labels() {
    use crate::widgets::prompt_center_panel::{test_rects, PromptCenterPanel};

    let panel_rect = test_rects::medium();
    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        state.editor_ui.open_prompt_center(1);
        state.editor_ui.prompt_center.save_open = true;
        let panel = PromptCenterPanel::for_editor(&state).expect("open");
        let what = format!("prompt center {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, panel_rect);
            panel_rect
        });
        let chips = panel
            .filter_chip_rects(panel_rect)
            .into_iter()
            .map(|(rect, _)| rect)
            .chain(
                panel
                    .save_category_rects(panel_rect)
                    .into_iter()
                    .map(|(rect, _)| rect),
            );
        for chip in chips {
            let run = run_starting_in(&backend, chip, &[], &what);
            assert_inside(run, chip, 0.0, &what);
        }
    }
}

#[test]
fn asset_center_chips_hold_their_labels() {
    use crate::widgets::asset_center_style_cards::style_test_support::exclusive_user_styles;
    use crate::widgets::scene_template_card_actions::BASIS_CHIP_MAX_W;
    use crate::widgets::scene_template_panel::{test_rects::MEDIUM, SceneTemplatePanel};

    let _guard = exclusive_user_styles();
    for locale in Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        state.editor_ui.scene_template_generate_supported = true;
        state.editor_ui.open_scene_template_center(0);
        state.editor_ui.scene_template_center.generate_basis = Some("minimal-keynote".into());
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        let what = format!("asset center templates {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, MEDIUM);
            MEDIUM
        });
        for (chip, _) in panel.filter_chip_rects(MEDIUM) {
            let run = run_starting_in(&backend, chip, &[], &what);
            assert_inside(run, chip, 0.0, &what);
        }
        let basis = panel.basis_chip_rect(MEDIUM).expect("a basis is set");
        let dismiss = panel.basis_chip_dismiss_rect(MEDIUM).expect("dismiss");
        let label_box = inner(basis, 0.0, dismiss.size.x);
        let run = run_starting_in(&backend, label_box, &[], &what);
        // Sized for its whole label, unless the cap that keeps the topic
        // field usable cut it — then ellipsized inside the capped chip.
        if basis.size.x < BASIS_CHIP_MAX_W {
            assert_eq!(run.text, panel.basis_chip_label().expect("label"), "{what}");
        }
        assert_inside(run, label_box, 0.0, &what);

        state.editor_ui.scene_template_center.tab = AssetCenterTab::Styles;
        let panel = SceneTemplatePanel::for_editor(&state).expect("open");
        let what = format!("asset center styles {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, MEDIUM);
            MEDIUM
        });
        let import = panel.style_import_button_rect(MEDIUM).expect("styles tab");
        let run = run_starting_in(&backend, import, &[], &what);
        assert_eq!(run.text, panel.style_import_label(), "{what}");
        assert_inside(run, import, 0.0, &what);
    }
}

#[test]
fn chat_chips_footer_and_header_hold_their_labels() {
    use crate::widgets::ai_chat_chip_row::{CHIP_CLEAR_W, CHIP_PAD_X};
    use crate::widgets::asset_center_style_cards::style_test_support::exclusive_user_styles;
    use crate::widgets::{AIChatPlaceholder, AI_CHAT_HEIGHT, AI_CHAT_WIDTH};

    let _guard = exclusive_user_styles();
    let pinned = de_ai_skills::style_guide::style_guide_registry()[0]
        .name
        .clone();
    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_HEIGHT);
    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        state.editor_ui.pinned_style_guide = Some(pinned.clone());
        state.selection.set = (0..3)
            .map(|index| de_editor_core::NodeId::new(format!("n{index}")))
            .collect();
        let mut panel = AIChatPlaceholder::from_editor_at(&state, 0);
        panel.composer_only = false;
        let what = format!("chat {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, rect);
            rect
        });

        let row = panel.chip_row(panel.input_rows_rect(rect));
        let selection = row.selection.expect("three nodes are selected");
        let label_box = inner(selection, CHIP_PAD_X, CHIP_CLEAR_W);
        // Two chips share the row, so either may be squeezed and ellipsized;
        // whole or cut, the label stays inside the chip the hit-test sees.
        let run = run_starting_in(&backend, label_box, &["×"], &what);
        assert_inside(run, label_box, 0.0, &what);
        if let Some(style) = row.style {
            let label_box = inner(style, CHIP_PAD_X, CHIP_CLEAR_W);
            let run = run_starting_in(&backend, label_box, &["×"], &what);
            assert_inside(run, label_box, 0.0, &what);
        }

        // The model chip (here the add-provider link, painted semibold).
        let footer = panel.footer_layout(rect);
        let label_box = inner(footer.model, 8.0, 22.0);
        let run = run_starting_in(&backend, label_box, &[], &what);
        assert_inside(run, label_box, 0.0, &what);

        let header = panel.header(rect);
        let label_box = inner(header.new_chat, 30.0, 12.0);
        let run = run_starting_in(&backend, label_box, &[], &what);
        assert_eq!(run.text, panel.label_new, "{what}");
        assert_inside(run, label_box, 0.0, &what);
    }
}

#[test]
fn minimized_chat_bar_sizes_its_model_column_from_the_painted_name() {
    use crate::widgets::ai_chat_panel_minimized::minimized_bar_layout;
    use crate::widgets::label_widths::LabelWidths;
    use crate::widgets::{AIChatPlaceholder, AI_CHAT_MINIMIZED_HEIGHT, AI_CHAT_WIDTH};

    let rect = Rect::xywh(0.0, 0.0, AI_CHAT_WIDTH, AI_CHAT_MINIMIZED_HEIGHT);
    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        state.chat.minimize();
        let mut panel = AIChatPlaceholder::from_editor_at(&state, 0);
        panel.composer_only = false;
        let what = format!("minimized chat {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, rect);
            rect
        });
        let layout = minimized_bar_layout(rect, &panel.label_no_models, &mut LabelWidths::cached());
        if layout.model.size.x > 0.0 {
            let run = run_starting_in(&backend, layout.model, &[], &what);
            assert_inside(run, layout.model, 0.0, &what);
        }
    }
}

#[test]
fn design_md_body_wraps_inside_the_panel_and_its_buttons_measure_in_inter() {
    use crate::widgets::design_md_panel::{DesignMdPanel, DESIGN_MD_PANEL_W};

    let markdown = "# Design System: Wrap\n\n## Visual Theme\n\
                    温暖厨房的设计说明使用柔和的阴影和圆角卡片以及宽松的留白间距，\
                    主色调为暖橙色并搭配中性灰色背景，整体风格安静而克制 \
                    and a long Latin sentence with `code spans` and #E07A5F tokens that \
                    must wrap at the width it is painted at rather than a character count\n";
    let rect = Rect::xywh(200.0, 80.0, DESIGN_MD_PANEL_W, 640.0);
    for locale in Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        state.editor_ui.design_md_panel.open = true;
        state.editor_ui.design_md_panel.expanded = 0xFF;
        state.doc.design_md = Some(de_editor_core::parse_design_md(markdown));
        let panel = DesignMdPanel::for_editor(&state).expect("open");
        let what = format!("design.md body {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[], |cx| {
            panel.paint(cx, rect);
            rect
        });
        // Body runs (11 px, below the header) end inside the panel's padding.
        let right = rect.origin.x + rect.size.x - 14.0;
        for run in backend
            .runs
            .iter()
            .filter(|run| run.font_size == 11.0 && run.origin.y > rect.origin.y + 70.0)
        {
            assert!(
                run.right_edge() <= right + 0.01,
                "{what}: {:?} ends at {} past {right}",
                run.text,
                run.right_edge()
            );
        }

        // The empty state's jian buttons measure their labels in the face
        // they draw them in (kemalcalak/jian#31).
        state.doc.design_md = None;
        let panel = DesignMdPanel::for_editor(&state).expect("open");
        let what = format!("design.md empty {locale:?}");
        let backend = assert_drawn_as_measured(&what, &[JIAN_COMPONENT_FAMILY], |cx| {
            panel.paint(cx, rect);
            rect
        });
        let buttons: Vec<&PaintedRun> = backend
            .runs
            .iter()
            .filter(|run| run.family == JIAN_COMPONENT_FAMILY)
            .collect();
        assert_eq!(
            buttons.len(),
            2,
            "{what}: both action buttons paint a label"
        );
        for run in buttons {
            assert!(
                backend
                    .styles_measured_for(&run.text)
                    .contains(&(JIAN_COMPONENT_FAMILY, 400)),
                "{what}: {:?} was not measured in {JIAN_COMPONENT_FAMILY}",
                run.text
            );
        }
    }
}
