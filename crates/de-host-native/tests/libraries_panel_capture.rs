//! Capture of the Components tab's Libraries section (#15): one library
//! synced from a real file on disk, one missing, painted through the real
//! native host. Asserts the section routes its buttons; set
//! `DESIGN_EDITOR_LIBRARIES_SHOT_DIR` to a folder to keep light, dark and
//! German PNGs of the rail for review.
//!
//! Windows-gated like every other test that solves layout through
//! `jian_skia::SkiaMeasure`.

#![cfg(all(feature = "gl-host", not(target_os = "windows")))]

use std::path::PathBuf;

use de_editor_core::{EditorState, LibraryLink, LibraryStatus, Locale, ThemeMode};
use de_host_native::backend::{NativeBackend, NativeFrameBackend};
use de_host_native::WidgetHostNative;

const VW: i32 = 1_200;
const VH: i32 = 760;

fn fixture_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("de-libraries-shot-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(dir.join("kits")).unwrap();
    let kit = serde_json::json!({
        "version": "1.0",
        "children": [
            { "id": "kit-button", "type": "frame", "name": "Button", "reusable": true,
              "width": 120, "height": 40 },
            { "id": "kit-badge", "type": "frame", "name": "Badge", "reusable": true,
              "width": 60, "height": 24 }
        ]
    });
    std::fs::write(dir.join("kits/brand.de"), kit.to_string()).unwrap();
    dir
}

fn linked_state(dir: &std::path::Path) -> EditorState {
    let mut state = EditorState::new();
    de_pen_loader::rebase_library_links(&mut state, Some(&dir.join("doc.de")));
    state.editor_ui.libraries.set_links(vec![
        LibraryLink::new("kits/brand.de", Some("Brand kit".into())),
        LibraryLink::new("kits/icons.de", None),
    ]);
    let problems = de_pen_loader::sync_libraries(&mut state);
    assert_eq!(problems, vec!["kits/icons.de".to_string()]);
    state
}

fn paint_rail(state: EditorState, out: Option<PathBuf>) {
    let mut host = WidgetHostNative::new();
    // Installing keeps the host's app preferences and drops editor-only
    // state, so the theme, locale and links go on afterwards.
    let ui = state.editor_ui.clone();
    host.install_imported_state(state);
    let live = &mut host.editor_state_mut().editor_ui;
    live.theme_mode = ui.theme_mode;
    live.locale = ui.locale;
    live.libraries = ui.libraries;
    live.open_components_tab();
    let mut backend = NativeBackend::with_dpi(1.0);
    let mut surface =
        skia_safe::surfaces::raster_n32_premul((VW, VH)).expect("raster surface allocated");
    for frame_ms in [1_000u64, 1_500] {
        host.set_now_ms(frame_ms);
        let mut frame = NativeFrameBackend::new(&mut backend, surface.canvas());
        host.paint(&mut frame, VW as f32, VH as f32);
    }
    if let Some(path) = out {
        let rail = surface
            .image_snapshot()
            .make_subset(
                None,
                skia_safe::IRect::from_xywh(0, 0, 360, 420),
                skia_safe::image::RequiredProperties::default(),
            )
            .expect("crop the rail");
        let data = rail
            .encode(None, skia_safe::EncodedImageFormat::PNG, 100)
            .expect("encode PNG");
        std::fs::write(&path, data.as_bytes()).expect("write the shot");
        eprintln!("libraries shot written to {}", path.display());
    }
}

#[test]
fn the_libraries_section_shows_synced_and_missing_files() {
    let dir = fixture_dir();
    let state = linked_state(&dir);
    assert_eq!(
        state.editor_ui.libraries.entries[0].status,
        LibraryStatus::Synced {
            components: 2,
            conflicts: 0
        }
    );
    assert_eq!(
        state.editor_ui.libraries.entries[1].status,
        LibraryStatus::Missing
    );

    let shots = std::env::var("DESIGN_EDITOR_LIBRARIES_SHOT_DIR")
        .ok()
        .map(PathBuf::from);
    let shot = |name: &str| shots.as_ref().map(|dir| dir.join(name));

    let mut light = state.clone();
    light.editor_ui.theme_mode = ThemeMode::Light;
    paint_rail(light, shot("libraries-light.png"));

    let mut dark = state.clone();
    dark.editor_ui.theme_mode = ThemeMode::Dark;
    paint_rail(dark, shot("libraries-dark.png"));

    let mut german = state;
    german.editor_ui.theme_mode = ThemeMode::Light;
    german.editor_ui.locale = Locale::De;
    paint_rail(german, shot("libraries-de.png"));

    let _ = std::fs::remove_dir_all(dir);
}
