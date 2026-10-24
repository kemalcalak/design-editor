//! Proof sheets for note / context / prompt annotations (#12): the sticky
//! notes on the canvas and the Annotation panel section, in light and dark,
//! plus one long-locale panel. Rendered through the real host paint with
//! the bundled fonts. Ignored by default because they write files; run with
//!
//!   DE_PROOF_DIR=/tmp/proof cargo test -p de-host-desktop note_proof -- --ignored

use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::NodeId;
use de_host_native::{NativeBackend, NativeFrameBackend, WidgetHostNative};

const W: f32 = 1440.0;
const H: f32 = 900.0;
const DPI: f32 = 2.0;

/// A small board with one of each annotation next to a frame.
const BOARD: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"card","name":"Pricing card","x":120,"y":140,"width":320,"height":220,
   "cornerRadius":16,"fill":[{"type":"solid","color":"#ffffff"}],"children":[
     {"type":"text","id":"t","content":"Pro plan","x":24,"y":24,"fontSize":22,"fontWeight":"700",
      "fill":[{"type":"solid","color":"#111111"}]}]},
  {"type":"note","id":"note","name":"Open questions","x":480,"y":140,"width":240,"height":170,
   "content":"Open questions: annual discount? Team seats above 30?"},
  {"type":"context","id":"ctx","name":"Brand voice","x":480,"y":340,"width":240,"height":150,
   "content":"Plain, friendly copy. No exclamation marks."},
  {"type":"prompt","id":"prompt","name":"Hero copy","x":760,"y":140,"width":260,"height":190,
   "content":"Write three headline options for the pricing hero.","model":"claude-opus"}
]}"##;

fn host(theme: ThemeMode, locale: de_i18n::Locale) -> WidgetHostNative {
    crate::bundled_fonts::register();
    let document = jian_ops_schema::load_str(BOARD)
        .expect("proof fixture parses")
        .value;
    let mut host = WidgetHostNative::new();
    host.install_imported_state(de_editor_core::EditorState::from_document(document));
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
fn note_proof_canvas_and_panel() {
    for (theme, suffix) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let mut h = host(theme, de_i18n::Locale::EnUs);
        shoot(&mut h, &format!("12-01-canvas-{suffix}"));
        h.editor_state_mut()
            .set_single_selection(NodeId::new("prompt"));
        shoot(&mut h, &format!("12-02-prompt-panel-{suffix}"));
        h.editor_state_mut()
            .set_single_selection(NodeId::new("note"));
        assert!(h.editor_state_mut().start_text_edit(NodeId::new("note")));
        shoot(&mut h, &format!("12-03-note-editing-{suffix}"));
    }
    let mut h = host(ThemeMode::Light, de_i18n::Locale::Tr);
    h.editor_state_mut()
        .set_single_selection(NodeId::new("prompt"));
    shoot(&mut h, "12-04-prompt-panel-tr");
    h.editor_state_mut().tool = de_editor_core::Tool::Note;
    shoot(&mut h, "12-05-note-tool-tr");
}
