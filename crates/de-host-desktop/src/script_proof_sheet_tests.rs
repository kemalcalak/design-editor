//! Proof sheets for the live script node (#19): a generated bar chart on
//! the canvas with the Script panel section, in light and dark, the code
//! editor open on the program, and one long-locale panel. Rendered through
//! the real host paint with the bundled fonts. Ignored by default because
//! they write files; run with
//!
//!   DE_PROOF_DIR=/tmp/proof cargo test -p de-host-desktop script_proof -- --ignored

use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::NodeId;
use de_host_native::{NativeBackend, NativeFrameBackend, WidgetHostNative};

const W: f32 = 1440.0;
const H: f32 = 900.0;
const DPI: f32 = 2.0;

const SOURCE: &str = "/** @schema 2.11\n * @input bars: number = 6\n * @input accent: color = #3b82f6\n * @input rounded: boolean = true\n * @input order: enum(random, rising) = random\n */\nconst n = pencil.input.bars;\nconst gap = 12;\nconst w = (pencil.width - gap * (n - 1)) / n;\nconst out = [];\nfor (let i = 0; i < n; i++) {\n  const t = pencil.input.order === 'rising' ? (i + 1) / n : 0.25 + Math.random() * 0.75;\n  const h = pencil.height * t;\n  out.push({ type: 'rectangle', x: i * (w + gap), y: pencil.height - h, width: w, height: h,\n             cornerRadius: pencil.input.rounded ? 8 : 0, fill: pencil.input.accent });\n}\nreturn out;";

fn host(theme: ThemeMode, locale: de_i18n::Locale) -> WidgetHostNative {
    crate::bundled_fonts::register();
    let board = serde_json::json!({"version": "1.0.0", "children": [
        {"type": "frame", "id": "card", "name": "Revenue", "x": 120, "y": 140, "width": 520,
         "height": 340, "cornerRadius": 16, "fill": [{"type": "solid", "color": "#ffffff"}],
         "children": [
            {"type": "script", "id": "chart", "name": "Bar chart", "x": 32, "y": 40,
             "width": 456, "height": 260, "clipContent": true, "script": SOURCE,
             "inputs": {"bars": 8}}]}]});
    let document = jian_ops_schema::load_str(&board.to_string())
        .expect("proof fixture parses")
        .value;
    let mut state = de_editor_core::EditorState::from_document(document);
    de_mcp::script_node::regenerate_stale_scripts(&mut state);
    let mut host = WidgetHostNative::new();
    host.install_imported_state(state);
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.locale = locale;
    ui.theme_mode = theme;
    ui.home.hide();
    host
}

fn shoot(host: &mut WidgetHostNative, name: &str) {
    let (pw, ph) = ((W * DPI) as i32, (H * DPI) as i32);
    let mut surface = skia_safe::surfaces::raster_n32_premul((pw, ph)).expect("raster surface");
    let mut backend = NativeBackend::with_dpi(DPI);
    for _ in 0..2 {
        let canvas = surface.canvas();
        canvas.reset_matrix();
        canvas.clear(skia_safe::Color::BLACK);
        canvas.scale((DPI, DPI));
        let mut frame = NativeFrameBackend::new(&mut backend, canvas);
        host.paint(&mut frame, W, H);
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
fn script_proof_canvas_and_panel() {
    for (theme, suffix) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let mut h = host(theme, de_i18n::Locale::EnUs);
        h.editor_state_mut()
            .set_single_selection(NodeId::new("chart"));
        shoot(&mut h, &format!("19-01-script-panel-{suffix}"));
    }
    let mut h = host(ThemeMode::Light, de_i18n::Locale::EnUs);
    h.editor_state_mut()
        .set_single_selection(NodeId::new("chart"));
    assert!(h.editor_state_mut().open_script_source_editor());
    shoot(&mut h, "19-02-script-source-editor");
    let mut h = host(ThemeMode::Light, de_i18n::Locale::Tr);
    h.editor_state_mut()
        .set_single_selection(NodeId::new("chart"));
    shoot(&mut h, "19-03-script-panel-tr");
}
