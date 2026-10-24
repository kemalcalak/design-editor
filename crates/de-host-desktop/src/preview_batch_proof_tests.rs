//! Proof sheets for the preview batch (#155 #157 #158 #176): the
//! Interactions / Motion panel rows and popovers, Present on an untagged
//! page, the Slides panel's Transition row and a push transition mid-way,
//! in light and dark, plus one long-locale panel. Rendered through the
//! real host paint with the bundled fonts. Ignored by default because
//! they write files; run with
//!
//!   DE_PROOF_DIR=/tmp/proof cargo test -p de-host-desktop preview_batch_proof -- --ignored

use de_editor_core::editor_ui_state::ThemeMode;
use de_editor_core::interaction_edit::{InteractionMenuTarget, InteractionTrigger};
use de_editor_core::{LeftPanelTab, NodeId};
use de_host_native::{NativeBackend, NativeFrameBackend, WidgetHostNative};

const W: f32 = 1440.0;
const H: f32 = 900.0;
const DPI: f32 = 2.0;

/// A landing page: header with a "Pricing" link, two named sections.
const SITE: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"page","name":"Landing","x":0,"y":0,"width":960,"height":900,
   "layout":"vertical","fill":[{"type":"solid","color":"#f6f4ef"}],"children":[
    {"type":"frame","id":"header","name":"Header","width":"fill_container","height":72,
     "layout":"horizontal","padding":[0,32],"gap":24,"alignItems":"center",
     "fill":[{"type":"solid","color":"#1d1b16"}],"children":[
       {"type":"text","id":"logo","content":"Northwind","fontSize":20,"fontWeight":"700",
        "fill":[{"type":"solid","color":"#ffffff"}]},
       {"type":"text","id":"link","content":"Pricing","fontSize":15,
        "fill":[{"type":"solid","color":"#e8e2d4"}],
        "events":{"onTap":[{"scroll_to":{"target":"pricing","alignment":"start"}}],
                  "onLongPress":[{"open_url":{"url":"\"https://example.com/docs\""}}]}}]},
    {"type":"frame","id":"hero","name":"Hero","width":"fill_container","height":360,
     "fill":[{"type":"solid","color":"#e9e3d6"}]},
    {"type":"frame","id":"pricing","name":"Pricing","width":"fill_container","height":420,
     "fill":[{"type":"solid","color":"#ffffff"}],"children":[
       {"type":"rectangle","id":"cta","name":"Buy button","x":40,"y":40,"width":180,"height":48,
        "cornerRadius":12,"fill":[{"type":"solid","color":"#3b5bfd"}],
        "transition":{"durationMs":180,"easing":"emphasized"},
        "animations":[{"trigger":"hover","durationMs":150,
          "keyframes":{"from":{"width":180},"to":{"width":200}}}]}]}]}
]}"##;

/// Three slides without any scenario tag: an ordinary page.
const SLIDES: &str = r##"{"version":"1.0.0","children":[
  {"type":"frame","id":"s1","name":"Cover","x":0,"y":0,"width":1600,"height":900,
   "fill":[{"type":"solid","color":"#1d3557"}],"children":[
     {"type":"text","id":"t1","content":"Quarterly review","x":120,"y":380,"fontSize":96,
      "fontWeight":"700","fill":[{"type":"solid","color":"#f1faee"}]}]},
  {"type":"frame","id":"s2","name":"Numbers","x":1800,"y":0,"width":1600,"height":900,
   "fill":[{"type":"solid","color":"#e63946"}],
   "slideTransition":{"kind":"push","durationMs":600,"easing":"linear"},"children":[
     {"type":"text","id":"t2","content":"Revenue +18%","x":120,"y":380,"fontSize":96,
      "fontWeight":"700","fill":[{"type":"solid","color":"#ffffff"}]}]},
  {"type":"frame","id":"s3","name":"Thanks","x":3600,"y":0,"width":1600,"height":900,
   "fill":[{"type":"solid","color":"#a8dadc"}],
   "slideTransition":{"kind":"push","durationMs":600,"easing":"linear"}}
]}"##;

fn host(source: &str, theme: ThemeMode) -> WidgetHostNative {
    crate::bundled_fonts::register();
    let document = jian_ops_schema::load_str(source)
        .expect("proof fixture parses")
        .value;
    let mut host = WidgetHostNative::new();
    host.install_imported_state(de_editor_core::EditorState::from_document(document));
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.locale = de_i18n::Locale::EnUs;
    ui.theme_mode = theme;
    ui.home.hide();
    host
}

/// Scroll the property panel to its end, where Interactions and Motion
/// sit (the panel clamps the offset to its content).
fn scroll_panel_to_end(host: &mut WidgetHostNative) {
    host.editor_state_mut()
        .editor_ui
        .property_panel_scroll
        .offset = 10_000.0;
}

fn shoot(host: &mut WidgetHostNative, name: &str) {
    let (pw, ph) = ((W * DPI) as i32, (H * DPI) as i32);
    let mut surface = skia_safe::surfaces::raster_n32_premul((pw, ph)).expect("raster surface");
    let mut backend = NativeBackend::with_dpi(DPI);
    // Two frames: the first settles layout-driven state (popover anchors,
    // slideshow camera), the second is the one kept.
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

fn suffix(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::Light => "light",
        _ => "dark",
    }
}

#[test]
#[ignore]
fn preview_batch_proof_interactions_panel() {
    for theme in [ThemeMode::Light, ThemeMode::Dark] {
        let s = suffix(theme);
        // #176: Scroll to / Open URL rows, the URL input, and the trigger
        // popover with its section links and Change to rows.
        let mut h = host(SITE, theme);
        h.editor_state_mut()
            .set_single_selection(NodeId::new("link"));
        scroll_panel_to_end(&mut h);
        shoot(&mut h, &format!("176-01-scroll-to-and-open-url-{s}"));
        h.editor_state_mut()
            .editor_ui
            .toggle_interaction_menu(InteractionMenuTarget::Trigger(InteractionTrigger::Tap));
        shoot(&mut h, &format!("176-02-trigger-popover-{s}"));

        // #176 + #155: node transition row and a hover size tween, with
        // the trigger popover open.
        let mut h = host(SITE, theme);
        h.editor_state_mut()
            .set_single_selection(NodeId::new("cta"));
        scroll_panel_to_end(&mut h);
        shoot(&mut h, &format!("155-01-motion-hover-width-{s}"));
        h.editor_state_mut()
            .editor_ui
            .toggle_interaction_menu(InteractionMenuTarget::MotionTrigger(0));
        shoot(&mut h, &format!("155-02-motion-trigger-popover-{s}"));
        h.editor_state_mut()
            .editor_ui
            .toggle_interaction_menu(InteractionMenuTarget::TransitionEasing);
        shoot(&mut h, &format!("176-03-transition-easing-popover-{s}"));
    }
    // A long locale: the same rows in German.
    let mut h = host(SITE, ThemeMode::Light);
    h.editor_state_mut().editor_ui.locale = de_i18n::Locale::De;
    h.editor_state_mut()
        .set_single_selection(NodeId::new("cta"));
    scroll_panel_to_end(&mut h);
    shoot(&mut h, "155-03-motion-panel-de-light");
    h.editor_state_mut()
        .set_single_selection(NodeId::new("link"));
    scroll_panel_to_end(&mut h);
    h.editor_state_mut()
        .editor_ui
        .toggle_interaction_menu(InteractionMenuTarget::Trigger(InteractionTrigger::Tap));
    shoot(&mut h, "176-04-trigger-popover-de-light");
}

#[test]
#[ignore]
fn preview_batch_proof_present_and_slide_transitions() {
    for theme in [ThemeMode::Light, ThemeMode::Dark] {
        let s = suffix(theme);
        // #158: the Slides panel's Transition row (the deck shares push
        // on two of three slides, so no cell is lit; select s2 to light it).
        let mut h = host(SLIDES, theme);
        {
            let ui = &mut h.editor_state_mut().editor_ui;
            ui.sidebar_open = true;
            ui.slides_panel.tab = LeftPanelTab::Slides;
        }
        h.editor_state_mut().set_single_selection(NodeId::new("s2"));
        shoot(&mut h, &format!("158-01-slides-panel-transition-row-{s}"));

        // #157: Present on an untagged page, then #158 push mid-way.
        let mut h = host(SLIDES, theme);
        h.set_now_ms(1_000);
        let (_x, _y, cw, ch) = (0.0, 0.0, W, H);
        assert!(h.toggle_present((cw, ch)), "Present starts on any page");
        shoot(&mut h, &format!("157-01-present-untagged-page-{s}"));
        assert!(h.preview_slideshow_step(1));
        h.set_now_ms(1_300);
        shoot(&mut h, &format!("158-02-push-transition-midway-{s}"));
        h.set_now_ms(2_000);
        shoot(&mut h, &format!("158-03-push-transition-settled-{s}"));
    }
}
