//! Modals and floating panels: command palette, rename layers, HTML import
//! diagnostics, Figma import, export dialog, design.md, icon picker and the
//! Settings Images / Fonts tabs, in all 15 locales.

use de_editor_core::{EditorState, NodeId};
use de_i18n::Locale;
use serde_json::json;

use super::assert_drawn_as_measured;
use crate::widgets::editor_state_ext::theme_for;
use crate::widgets::Widget;
use crate::Rect;

const VW: f32 = 1440.0;
const VH: f32 = 900.0;

fn document_state(locale: Locale) -> EditorState {
    let doc = json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "screen", "name": "Frame 1", "width": 1440, "height": 900,
         "children": [
            {"type": "frame", "id": "cta", "name": "Rectangle 3", "width": 160, "height": 48,
             "fill": [{"type": "solid", "color": "#2563EB"}],
             "children": [{"type": "text", "id": "label", "name": "Text",
                           "content": "Start your free trial today", "fontSize": 16}]}
         ]}
    ]});
    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(&doc.to_string())
            .expect("fixture")
            .value,
    );
    state.editor_ui.locale = locale;
    state
}

#[test]
fn command_palette_rows_and_find_results_paint_in_the_chrome_family() {
    use crate::widgets::command_palette_panel::CommandPalettePanel;
    use de_editor_core::command_palette::{command_palette_text, PaletteMode};

    for mode in [PaletteMode::Commands, PaletteMode::FindLayers] {
        for locale in Locale::ALL {
            let mut state = document_state(locale);
            state.toggle_command_palette(mode, 0);
            for c in "st".chars() {
                command_palette_text(&mut state, c, 0);
            }
            let panel = CommandPalettePanel::for_editor(&state).expect("open");
            let rect = panel.rect(VW, VH);
            assert_drawn_as_measured(&format!("palette {mode:?} {locale:?}"), &[], |cx| {
                panel.paint_at(cx, rect, 0);
                rect
            });
        }
    }
}

#[test]
fn rename_layers_rows_paint_in_the_chrome_family() {
    use crate::widgets::rename_layers_panel::RenameLayersPanel;

    for locale in Locale::ALL {
        let mut state = document_state(locale);
        state.set_single_selection(NodeId::new("cta"));
        assert!(state.open_rename_layers());
        let panel = RenameLayersPanel::for_editor(&state).expect("open");
        let rect = panel.rect(VW, VH);
        assert_drawn_as_measured(&format!("rename layers {locale:?}"), &[], |cx| {
            panel.paint_at(cx, rect, 0);
            rect
        });
    }
}

#[test]
fn html_import_diagnostics_wrap_in_the_chrome_family() {
    use crate::widgets::html_import_diagnostics_flow::publish;
    use crate::widgets::html_import_diagnostics_panel::HtmlImportDiagnosticsPanel;
    use de_editor_core::html_import_diagnostics::HtmlImportDiagnostic;

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        let rows = (0..3)
            .map(|index| {
                HtmlImportDiagnostic::new(
                    "layout.float_ignored",
                    "htmlImport.warn.layout.float_ignored".to_string(),
                    Vec::new(),
                    format!("CSS float ignored during structured HTML import {index}"),
                )
            })
            .collect();
        publish(&mut state, rows);
        let panel = HtmlImportDiagnosticsPanel::for_editor(&state).expect("open");
        let rect = panel.rect(VW, VH);
        assert_drawn_as_measured(&format!("html diagnostics {locale:?}"), &[], |cx| {
            panel.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn import_dialogs_centre_their_copy_in_the_chrome_family() {
    use crate::widgets::figma_import::FigmaImportModal;
    use crate::widgets::figma_import_progress::ImportProgressOverlay;

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        let modal = FigmaImportModal::for_editor(&state);
        let rect = modal.rect(VW, VH);
        assert_drawn_as_measured(&format!("figma import {locale:?}"), &["Inter"], |cx| {
            modal.paint(cx, rect);
            rect
        });
        let progress = ImportProgressOverlay::for_editor(&state, 0);
        let rect = progress.rect(VW, VH);
        assert_drawn_as_measured(&format!("import progress {locale:?}"), &[], |cx| {
            progress.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn export_dialog_paints_in_the_chrome_family() {
    use crate::widgets::export_dialog::ExportDialog;

    for locale in Locale::ALL {
        let state = document_state(locale);
        let dialog = ExportDialog::centered(VW, VH);
        let theme = theme_for(&state.editor_ui);
        assert_drawn_as_measured(&format!("export dialog {locale:?}"), &["Inter"], |cx| {
            dialog.paint(cx.backend, &theme, &state.editor_ui);
            dialog.rect()
        });
    }
}

#[test]
fn design_md_panel_lays_runs_out_at_their_painted_width() {
    use crate::widgets::design_md_panel::{DesignMdPanel, DESIGN_MD_PANEL_W};

    let markdown = "# Design System: Long names\n\n## Visual Theme\nWarm **bold** and `code` \
                    runs side by side\n\n## Color Palette\n\
                    - **A very long colour name that will not fit** (#2563EB) - primary call to action\n";
    for locale in Locale::ALL {
        let mut state = EditorState::default();
        state.editor_ui.locale = locale;
        state.editor_ui.design_md_panel.open = true;
        state.editor_ui.design_md_panel.expanded = 0xFF;
        state.doc.design_md = Some(de_editor_core::parse_design_md(markdown));
        let panel = DesignMdPanel::for_editor(&state).expect("open");
        let rect = Rect::xywh(200.0, 80.0, DESIGN_MD_PANEL_W, 640.0);
        let backend = assert_drawn_as_measured(&format!("design.md {locale:?}"), &[], |cx| {
            panel.paint(cx, rect);
            rect
        });
        // Runs on one line abut: each starts where the one before it ends
        // at its painted width, never at a per-character estimate.
        for pair in backend.runs.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if a.origin.y == b.origin.y && a.text == "bold" {
                assert!(
                    (b.origin.x - a.right_edge()).abs() < 0.01,
                    "{locale:?}: {:?} starts at {} but {:?} ends at {}",
                    b.text,
                    b.origin.x,
                    a.text,
                    a.right_edge(),
                );
            }
        }
    }
}

#[test]
fn icon_picker_rows_fit_in_the_chrome_family() {
    use crate::widgets::host_overlay_geometry::icon_picker_panel_rect;
    use crate::widgets::IconPickerPanel;

    for locale in Locale::ALL {
        let mut state = EditorState::new();
        state.editor_ui.locale = locale;
        state.editor_ui.icon_picker.open = true;
        let panel = IconPickerPanel::for_editor(&state).expect("open");
        let rect = icon_picker_panel_rect(&state, VW, VH).expect("open");
        assert_drawn_as_measured(&format!("icon picker {locale:?}"), &["Inter"], |cx| {
            panel.paint(cx, rect);
            rect
        });
    }
}

#[test]
fn settings_images_and_fonts_tabs_paint_in_the_chrome_family() {
    use crate::widgets::agent_settings_panel::AgentSettingsPanel;
    use de_editor_core::missing_fonts::{MissingFontEntry, MissingFontsPrompt};
    use de_editor_core::AgentSettingsTab;

    for tab in [AgentSettingsTab::Images, AgentSettingsTab::Fonts] {
        for locale in Locale::ALL {
            let mut state = EditorState::default();
            state.editor_ui.locale = locale;
            state.editor_ui.agent_settings.tab = tab;
            state.editor_ui.imported_font_families =
                std::sync::Arc::new(vec!["Playfair Display Extra Condensed SemiBold".to_string()]);
            state.editor_ui.missing_fonts_prompt = Some(MissingFontsPrompt {
                entries: vec![MissingFontEntry {
                    family: "Neue Haas Grotesk Display Pro".to_string(),
                    run_count: 12,
                    mismatch_note: None,
                    resolved: false,
                }],
            });
            let panel = AgentSettingsPanel::for_editor(&state);
            let rect = panel.rect(1200.0, 800.0);
            assert_drawn_as_measured(&format!("settings {tab:?} {locale:?}"), &[], |cx| {
                panel.paint(cx, rect);
                rect
            });
        }
    }
}
