//! Floating chrome: tooltip, toast, menus, IME bubble, drop overlay and the
//! canvas frame labels, in all 15 locales.

use de_editor_core::editor_toast::EditorToastLevel;
use de_editor_core::{EditorState, EditorUiState};
use de_i18n::Locale;

use super::assert_drawn_as_measured;
use crate::layout_scene::{NodeKind, SceneNode};
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::label_widths::LabelWidths;
use crate::widgets::{PaintCx, Widget};
use crate::{Color, Point2D, Rect};

/// jian's `Select` / `Menu` paint their rows in their own face.
const JIAN_COMPONENT_FAMILY: &str = "Inter";

fn ui_in(locale: Locale) -> EditorUiState {
    EditorUiState {
        locale,
        ..Default::default()
    }
}

#[test]
fn tooltip_bubble_holds_its_label_in_the_painted_family() {
    use crate::widgets::tooltip::{paint_tooltip, TooltipPlacement};

    for locale in Locale::ALL {
        let ui = ui_in(locale);
        let label = translate(&ui, "toolbar.select");
        let anchor = Rect::xywh(400.0, 300.0, 32.0, 32.0);
        assert_drawn_as_measured(&format!("tooltip {locale:?}"), &[], |cx| {
            paint_tooltip(
                cx,
                &theme_for(&ui),
                anchor,
                label,
                Some("⌘K"),
                TooltipPlacement::Below,
                None,
            )
        });
    }
}

#[test]
fn toast_banner_is_sized_from_its_message_in_the_painted_family() {
    use crate::widgets::editor_toast_flow::toast_rect;

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        state.editor_ui.show_toast(
            "menu.saveAsTemplate.failed",
            Vec::new(),
            EditorToastLevel::Warn,
            0,
        );
        assert_drawn_as_measured(&format!("toast {locale:?}"), &[], |cx| {
            let (toast, rect) = toast_rect(cx, &state, 1440.0, 900.0, 0).expect("visible");
            toast.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn import_menu_fits_its_shortcuts_in_the_painted_family() {
    use crate::widgets::import_menu::ImportMenu;

    for locale in Locale::ALL {
        let ui = EditorUiState {
            locale,
            import_menu_open: true,
            import_menu: jian_widgets::components::select::SelectState {
                open: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let menu = ImportMenu::for_editor_ui(&ui);
        let anchor = Rect::xywh(100.0, 40.0, 120.0, 28.0);
        let viewport = Rect::xywh(0.0, 0.0, 1440.0, 900.0);
        assert_drawn_as_measured(
            &format!("import menu {locale:?}"),
            &[JIAN_COMPONENT_FAMILY],
            |cx| {
                menu.paint_select(cx, anchor, viewport);
                menu.popup_rect(anchor, viewport)
            },
        );
    }
}

#[test]
fn shape_picker_and_export_menu_paint_in_the_chrome_family() {
    use crate::widgets::export_quick_menu::ExportQuickMenu;
    use crate::widgets::shape_picker::ShapePicker;

    for locale in Locale::ALL {
        let ui = ui_in(locale);
        let picker = ShapePicker::for_editor_ui(&ui);
        let rect = Rect::xywh(60.0, 120.0, 220.0, ShapePicker::panel_height());
        assert_drawn_as_measured(
            &format!("shape picker {locale:?}"),
            &[JIAN_COMPONENT_FAMILY],
            |cx| {
                picker.paint(cx, rect);
                rect
            },
        );

        let menu = ExportQuickMenu::for_editor_ui(&ui);
        let rect = menu.rect_at(Rect::xywh(1200.0, 14.0, 32.0, 32.0), 1440.0);
        assert_drawn_as_measured(&format!("export menu {locale:?}"), &[], |cx| {
            menu.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn path_anchor_menu_paints_in_the_chrome_family() {
    use crate::widgets::path_anchor_context_menu::PathAnchorContextMenu;
    use de_editor_core::{NodeId, PathAnchorMenuState};

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        let menu = PathAnchorContextMenu::for_state(
            &state,
            PathAnchorMenuState {
                node_id: NodeId::new("path"),
                anchor_index: 0,
                x: 300.0,
                y: 200.0,
                menu: Default::default(),
            },
        );
        assert_drawn_as_measured(&format!("path anchor menu {locale:?}"), &[], |cx| {
            menu.paint(cx);
            menu.rect()
        });
    }
}

#[test]
fn theme_preset_menu_paints_in_the_chrome_family() {
    use crate::widgets::variables_preset_menu::ThemePresetMenu;
    use jian_ops_schema::variable::{VariableKind, VariableScalar};

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        assert!(state.create_variable(
            "brand",
            VariableKind::Color,
            VariableScalar::Str("#ff0000".into()),
        ));
        assert!(state.save_theme_preset("A long preset name for the menu", 10));
        state.editor_ui.variables_preset_menu_open = true;
        let menu = ThemePresetMenu::for_editor(&state);
        let rect = menu.menu_rect(Rect::xywh(100.0, 50.0, 122.0, 32.0));
        assert_drawn_as_measured(&format!("theme preset menu {locale:?}"), &[], |cx| {
            menu.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn ime_bubble_and_drop_overlay_paint_in_the_chrome_family() {
    use crate::widgets::file_drop_overlay::paint_file_drop_overlay;
    use crate::widgets::ime_preedit_overlay::paint_ime_preedit;

    let viewport = Rect::xywh(0.0, 0.0, 1440.0, 900.0);
    for locale in Locale::ALL {
        let ui = ui_in(locale);
        let theme = theme_for(&ui);
        assert_drawn_as_measured(&format!("ime preedit {locale:?}"), &[], |cx| {
            paint_ime_preedit(
                cx,
                &theme,
                viewport,
                Some(Rect::xywh(200.0, 200.0, 240.0, 28.0)),
                "にほんご input",
                Some((0, 9)),
            );
            viewport
        });
        assert_drawn_as_measured(&format!("file drop overlay {locale:?}"), &[], |cx| {
            paint_file_drop_overlay(cx.backend, &theme, locale, viewport, None);
            viewport
        });
    }
}

/// The frame label's hit box is sized from the name as painted (#441): it
/// used to be seven pixels a character, so a wide name stuck out of its
/// own clickable box and a narrow one claimed canvas beside it.
#[test]
fn frame_label_hit_box_covers_the_painted_name() {
    use crate::widgets::canvas_frame_labels::{
        frame_label_at_point, paint_frame_labels, FrameLabel,
    };
    use de_editor_core::Viewport;

    let clip = Rect::xywh(0.0, 0.0, 1440.0, 900.0);
    let mut frame = SceneNode::leaf("n1", NodeKind::Frame);
    frame.bounds = Rect::xywh(40.0, 120.0, 800.0, 400.0);
    let roots = [frame];
    let viewport = Viewport::IDENTITY;
    for name in [
        "Frame",
        "Wide Marketing Landing Page — Desktop",
        "设计系统首页",
        "Главная страница",
    ] {
        let labels = vec![FrameLabel::new("n1", name, Color::WHITE, false)];
        let paint = |cx: &mut PaintCx<'_>| {
            paint_frame_labels(cx, &roots, &labels, &[], Point2D::ZERO, &viewport, clip);
            clip
        };
        let backend = assert_drawn_as_measured(&format!("frame label {name:?}"), &[], paint);
        let run = &backend.runs[0];
        let y = 120.0 - 12.0;
        let hit = |x: f32| {
            frame_label_at_point(
                &roots,
                &labels,
                Point2D::ZERO,
                &viewport,
                clip,
                Point2D::new(x, y),
            )
        };
        assert_eq!(hit(run.right_edge() - 0.5), Some("n1".into()), "{name}");
        assert_eq!(hit(run.right_edge() + 8.0), None, "{name}");
        assert_eq!(
            LabelWidths::cached().chrome_weighted(name, 12.0, 500),
            run.width_in_paint_family(),
            "{name}: the hit-test reads the painted width"
        );
    }
}
