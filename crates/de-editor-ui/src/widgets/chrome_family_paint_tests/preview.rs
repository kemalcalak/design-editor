//! Preview chrome (device switcher, browser bar, screen pills, slideshow
//! toolbar) and the left rail's slides tab, in all 15 locales.

use de_editor_core::{EditorState, EditorUiState};
use de_i18n::Locale;

use super::assert_drawn_as_measured;
use crate::widgets::editor_state_ext::{theme_for, translate};
use crate::widgets::label_fit_test_support::{assert_centred, assert_inside};
use crate::Rect;

const CANVAS: Rect = Rect {
    origin: crate::Point2D { x: 0.0, y: 0.0 },
    size: crate::Point2D {
        x: 1440.0,
        y: 900.0,
    },
};

fn ui_in(locale: Locale) -> EditorUiState {
    EditorUiState {
        locale,
        ..Default::default()
    }
}

#[test]
fn device_switcher_centres_each_label_in_its_segment() {
    use crate::widgets::preview_device_switcher::PreviewDeviceSwitcher;

    for locale in Locale::ALL {
        let ui = ui_in(locale);
        let switcher = PreviewDeviceSwitcher {
            labels: [
                translate(&ui, "preview.device.phone"),
                translate(&ui, "preview.device.desktop"),
                translate(&ui, "preview.device.canvas"),
            ],
            selected: None,
            hover: None,
            pressed: None,
        };
        let theme = theme_for(&ui);
        let backend = assert_drawn_as_measured(&format!("device switcher {locale:?}"), &[], |cx| {
            switcher.paint(cx, CANVAS, &theme);
            PreviewDeviceSwitcher::pill_rect(CANVAS)
        });
        for (index, (_, segment)) in PreviewDeviceSwitcher::segment_rects(CANVAS)
            .into_iter()
            .enumerate()
        {
            let run = &backend.runs[index];
            let label = switcher.labels[index];
            assert!(
                run.text == label || run.text.ends_with('…'),
                "{locale:?}: {:?} is neither {label:?} nor its fitted form",
                run.text
            );
            let what = format!("device switcher {locale:?}");
            assert_inside(run, segment, 0.0, &what);
            assert_centred(run, segment, &what);
        }
    }
}

#[test]
fn browser_bar_and_screen_pills_fit_in_the_chrome_family() {
    use crate::widgets::preview_browser_bar::PreviewBrowserBar;
    use crate::widgets::screen_switcher_pills::ScreenSwitcherPills;

    let labels = vec![
        "Home".to_string(),
        "Pricing and plans for every team size".to_string(),
        "设计系统".to_string(),
        "Регистрация".to_string(),
    ];
    for locale in Locale::ALL {
        let ui = ui_in(locale);
        let theme = theme_for(&ui);
        let bar = PreviewBrowserBar {
            title: "Pricing and plans for every team size",
            route: "/pricing/plans/enterprise-and-education",
            can_go_back: true,
            can_go_forward: false,
            hover: None,
            pressed: None,
            link_hints: false,
        };
        assert_drawn_as_measured(&format!("browser bar {locale:?}"), &[], |cx| {
            bar.paint(cx, CANVAS, &theme);
            PreviewBrowserBar::bar_rect(CANVAS)
        });

        let pills = ScreenSwitcherPills {
            labels: &labels,
            active: Some(1),
            hover: None,
        };
        let backend = assert_drawn_as_measured(&format!("screen pills {locale:?}"), &[], |cx| {
            pills.paint(cx, CANVAS, &theme);
            ScreenSwitcherPills::row_rect(CANVAS, labels.len())
        });
        // Each label (fitted or whole) stays inside the pill the
        // backend-less hit-test resolves for it.
        for (run, pill) in backend
            .runs
            .iter()
            .zip(ScreenSwitcherPills::pill_rects(CANVAS, labels.len()))
        {
            assert_inside(run, pill, 0.0, &format!("screen pill {locale:?}"));
        }
    }
}

#[test]
fn slideshow_counter_sits_between_its_buttons() {
    use crate::widgets::slideshow_toolbar::SlideshowToolbar;
    use de_editor_core::preview_slideshow::SlideshowToolbarButton;

    for label in ["3 / 6", "12 / 128"] {
        let toolbar = SlideshowToolbar {
            label,
            can_go_back: true,
            can_go_forward: true,
            hover: None,
            pressed: None,
        };
        let theme = theme_for(&EditorUiState::default());
        let backend = assert_drawn_as_measured(&format!("slideshow {label}"), &[], |cx| {
            toolbar.paint(cx, CANVAS, &theme);
            SlideshowToolbar::pill_rect(CANVAS, label)
        });
        let rects = SlideshowToolbar::button_rects(CANVAS, label);
        let previous = rects[0].1;
        let next = rects[1].1;
        assert_eq!(rects[1].0, SlideshowToolbarButton::Next);
        let counter = Rect::xywh(
            previous.origin.x + previous.size.x,
            previous.origin.y,
            next.origin.x - (previous.origin.x + previous.size.x),
            previous.size.y,
        );
        let run = backend
            .runs
            .iter()
            .find(|run| run.text == label)
            .expect("counter painted");
        assert_centred(run, counter, &format!("slideshow {label}"));
    }
}

#[test]
fn slides_tab_paints_in_the_chrome_family() {
    use crate::layout_scene::{LayoutScene, NodeKind, SceneNode, ScenePage};
    use crate::widgets::slides_panel_flow as flow;
    use de_editor_core::scene_template_catalog::TemplateScene;
    use de_editor_core::LeftPanelTab;

    let source = r#"{"version":"1.0.0","children":[
        {"type":"frame","id":"slide-1","name":"Cover","x":0,"y":0,"width":1920,"height":1080},
        {"type":"frame","id":"slide-2","name":"Agenda","x":2100,"y":0,"width":1920,"height":1080}
    ]}"#;
    let board = |id: &str, x: f32| {
        let mut node = SceneNode::leaf(id, NodeKind::Frame);
        node.bounds = Rect::xywh(x, 0.0, 1920.0, 1080.0);
        node
    };
    let scene = LayoutScene {
        pages: vec![ScenePage {
            id: "page-1".into(),
            name: "Page 1".into(),
            children: vec![board("slide-1", 0.0), board("slide-2", 2100.0)],
            image_thumbs: Default::default(),
        }],
        active_page_index: 0,
    };
    let panel = Rect::xywh(0.0, 48.0, 240.0, 700.0);
    for locale in Locale::ALL {
        let document = jian_ops_schema::load_str(source).expect("fixture").value;
        let mut state = EditorState::from_document(document);
        state.editor_ui.locale = locale;
        state.editor_ui.scenario = Some(TemplateScene::Slides);
        state.editor_ui.slides_panel.tab = LeftPanelTab::Slides;
        let chips = flow::slides(&state);
        let layout = flow::layout(&state, &chips, &scene, panel).expect("room");
        let tabs = flow::tab_row_desc(&state);
        let text = flow::action_labels(&state, flow::selected_slide_count(&state, &chips));
        let widget = flow::widget(Some(0), &state, &tabs, text.labels());
        let theme = theme_for(&state.editor_ui);
        assert_drawn_as_measured(&format!("slides tab {locale:?}"), &["Inter"], |cx| {
            widget.paint(cx, &layout, &theme);
            widget.paint_overlay(cx, &layout, &theme);
            panel
        });
    }
}
