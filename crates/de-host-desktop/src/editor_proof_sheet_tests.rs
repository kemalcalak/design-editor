//! Proof sheets for the editor interior (#30): the chrome around a real
//! document — left rail, floating file pill / tool rail / action group,
//! right panel — rendered with the bundled fonts to PNGs for a visual
//! check against the design frames. Ignored by default because they write
//! files; run with
//!
//!   DE_PROOF_DIR=/tmp/proof cargo test -p de-host-desktop editor_proof -- --ignored
//!
//! (without `DE_PROOF_DIR` the sheets land in the system temp directory).

use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::{LeftPanelTab, NodeId};
use de_editor_ui::Rect;
use de_host_native::{NativeBackend, NativeFrameBackend, WidgetHostNative};

const DPI: f32 = 2.0;

/// The landing page fixture at the repository root — a real design with
/// text, images and nested auto layout, not a synthetic box.
fn fixture() -> de_editor_core::EditorState {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../lumina-landing.de");
    let json = std::fs::read_to_string(path).expect("fixture readable");
    let doc = jian_ops_schema::load_str(&json)
        .expect("fixture parses")
        .value;
    de_editor_core::EditorState::from_document(doc)
}

fn host(theme: ThemeMode, w: f32, h: f32, tab: LeftPanelTab) -> WidgetHostNative {
    crate::bundled_fonts::register();
    let mut host = WidgetHostNative::new();
    let mut state = fixture();
    {
        let ui = &mut state.editor_ui;
        ui.locale = de_i18n::Locale::EnUs;
        ui.theme_mode = theme;
        ui.home.hide();
        ui.file_name_display = Some("lumina-landing.de".to_string());
        match tab {
            LeftPanelTab::Chat => {
                ui.enter_chat_tab();
            }
            LeftPanelTab::Components => {
                ui.open_components_tab();
            }
            other => ui.slides_panel.tab = other,
        }
    }
    state.set_single_selection(NodeId::new("n1"));
    let (_x, _y, cw, ch) = de_editor_ui::widgets::host_canvas_geometry::canvas_region(&state, w, h);
    state
        .viewport
        .fit_to_with_max_zoom(Rect::xywh(0.0, 0.0, 1440.0, 900.0), cw, ch, 96.0, 1.0);
    *host.editor_state_mut() = state;
    host
}

fn shoot(host: &mut WidgetHostNative, w: f32, h: f32, name: &str) {
    let (pw, ph) = ((w * DPI) as i32, (h * DPI) as i32);
    let mut surface = skia_safe::surfaces::raster_n32_premul((pw, ph)).expect("raster surface");
    let mut backend = NativeBackend::with_dpi(DPI);
    // Two passes: the first resolves images and caches that the second
    // paints, so the sheet shows the settled frame.
    for _ in 0..2 {
        let canvas = surface.canvas();
        canvas.reset_matrix();
        canvas.clear(skia_safe::Color::BLACK);
        canvas.scale((DPI, DPI));
        let mut frame = NativeFrameBackend::new(&mut backend, canvas);
        host.paint(&mut frame, w, h);
    }
    let data = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
        .expect("encode png");
    let dir = std::env::var_os("DE_PROOF_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).expect("proof dir");
    let out = dir.join(format!("{name}.png"));
    std::fs::write(&out, data.as_bytes()).expect("write png");
    println!("proof sheet written to {}", out.display());
}

#[test]
#[ignore]
fn editor_proof_sheets() {
    for (theme, suffix) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        for (w, h, size) in [(1440.0, 900.0, "1440"), (1024.0, 700.0, "1024")] {
            for (tab, name) in [
                (LeftPanelTab::Layers, "layers"),
                (LeftPanelTab::Chat, "agent"),
                (LeftPanelTab::Components, "components"),
                (LeftPanelTab::Images, "images"),
            ] {
                let mut h_ = host(theme, w, h, tab);
                shoot(&mut h_, w, h, &format!("editor-{name}-{size}-{suffix}"));
            }
            // Right panel: its foot (the Code chips) and the Code view.
            let mut h_ = host(theme, w, h, LeftPanelTab::Layers);
            h_.editor_state_mut().editor_ui.property_panel_scroll.offset = 10_000.0;
            shoot(&mut h_, w, h, &format!("editor-panel-foot-{size}-{suffix}"));
            let mut h_ = host(theme, w, h, LeftPanelTab::Layers);
            h_.editor_state_mut().editor_ui.property_tab = de_editor_core::PropertyTab::Code;
            shoot(&mut h_, w, h, &format!("editor-code-view-{size}-{suffix}"));
        }
    }
}
