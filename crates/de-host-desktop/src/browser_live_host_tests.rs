//! The desktop live host with a scripted driver: commands reach the
//! drivers, decoded frames land under the node's stable texture, a
//! deselected node keeps its last frame and the setting off drops it. The scheduling rules are
//! tested on the shared queue
//! (`de_editor_ui::widgets::browser_stream_queue`).

use std::sync::atomic::Ordering;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};

use de_editor_core::EditorState;
use de_editor_ui::widgets::browser_render_queue::BrowserRenderQueue;
use de_editor_ui::widgets::browser_textures;
use de_host_native::NativeBackend;
use skia_safe::ConditionallySend;

use super::{BrowserLiveHost, DriverArgs, DriverControl, LiveFrame};

/// The paint registry is process-wide: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    crate::browser_live_host::browser_registry_test_lock()
}

fn frame_image() -> skia_safe::Sendable<skia_safe::Image> {
    let info = skia_safe::ImageInfo::new_n32_premul((4, 3), None);
    let pixels = skia_safe::Data::new_copy(&[0x80; 4 * 3 * 4]);
    skia_safe::images::raster_from_data(&info, pixels, 4 * 4)
        .unwrap()
        .wrap_send()
        .ok()
        .unwrap()
}

/// A host with a scripted driver over a one-browser document.
struct Rig {
    id: &'static str,
    host: BrowserLiveHost,
    /// The drivers the host started, kept so the test can feed frames.
    started: Arc<Mutex<Vec<DriverArgs>>>,
    state: EditorState,
    snapshots: BrowserRenderQueue,
    backend: NativeBackend,
}

impl Rig {
    fn new(id: &'static str, dpi: f32) -> Self {
        let doc = serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "children": [{ "type": "browser", "id": id, "width": 400, "height": 300,
                           "url": "https://example.com" }]
        }))
        .expect("document");
        let mut state = EditorState::new();
        state.replace_document(doc);
        state.set_single_selection(de_editor_core::NodeId::new(id));
        let started: Arc<Mutex<Vec<DriverArgs>>> = Arc::default();
        let sink = started.clone();
        let host = BrowserLiveHost::with_spawn(Arc::new(move |args| {
            sink.lock().unwrap().push(args);
        }));
        Self {
            id,
            host,
            started,
            state,
            snapshots: BrowserRenderQueue::new(2),
            backend: NativeBackend::with_dpi(dpi),
        }
    }

    /// Paint a canvas frame showing the node at `size` (or not at all),
    /// then pump the snapshot queue and the host.
    fn frame(&mut self, size: Option<[f32; 2]>, dpi: f32, active: bool) -> bool {
        browser_textures::begin_frame(false);
        if let Some(size) = size {
            browser_textures::note_visible(self.id, size);
        }
        self.snapshots.pump(&mut self.state, 0.0);
        self.host
            .pump(&self.state, &self.snapshots, dpi, active, &mut self.backend)
    }

    /// Feed driver `index` a frame numbered `seq`.
    fn send_frame(&self, index: usize, seq: u64) {
        let drivers = self.started.lock().unwrap();
        let args = &drivers[index];
        args.taken.store(false, Ordering::Release);
        args.frames
            .send(LiveFrame {
                node_id: args.node_id.clone(),
                stream: args.stream,
                seq,
                image: frame_image(),
            })
            .unwrap();
    }

    /// The next message driver `index` got, past the `Taken` wake-ups.
    fn next_control(&self, index: usize) -> Result<DriverControl, TryRecvError> {
        let drivers = self.started.lock().unwrap();
        loop {
            match drivers[index].control.try_recv() {
                Ok(DriverControl::Taken) => {}
                other => return other,
            }
        }
    }

    fn texture(&self) -> u64 {
        browser_textures::live_texture(self.id)
    }
}

const SHOWN: Option<[f32; 2]> = Some([400.0, 300.0]);

#[test]
fn frames_land_under_one_texture_and_a_deselect_keeps_the_last() {
    let _serial = serial();
    let mut rig = Rig::new("lh-node", 2.0);
    rig.frame(SHOWN, 2.0, true);
    assert_eq!(rig.host.streaming(), 1);
    assert_eq!(
        rig.started.lock().unwrap()[0].spec.frame_width,
        1024,
        "400 px at 2×"
    );
    rig.send_frame(0, 1);
    rig.send_frame(0, 2);
    assert!(rig.host.install_frames(&mut rig.backend));
    let texture = rig.texture();
    assert_eq!(browser_textures::live_frame(rig.id), Some((texture, 2)));
    assert!(rig.backend.image_resident(texture));
    assert!(rig.started.lock().unwrap()[0].taken.load(Ordering::Acquire));
    assert!(!rig.host.install_frames(&mut rig.backend), "nothing new");

    // Deselected: the stream stops, its last frame stays up.
    rig.state.clear_selection();
    assert!(!rig.frame(SHOWN, 2.0, true), "nothing to repaint");
    assert_eq!(rig.host.streaming(), 0);
    assert_eq!(
        browser_textures::live_for_paint(rig.id),
        Some((texture, false))
    );
    assert!(rig.backend.image_resident(texture));
    let stopped = rig.next_control(0);
    assert!(
        matches!(stopped, Err(TryRecvError::Disconnected)),
        "the driver is told to stop"
    );
    // The stopped driver's late frame is dropped.
    rig.send_frame(0, 3);
    assert!(!rig.host.install_frames(&mut rig.backend));
    assert_eq!(browser_textures::live_frame(rig.id), Some((texture, 2)));
    // The next still replaces the kept frame.
    browser_textures::install(rig.id, 7, b"PNG".to_vec());
    assert_eq!(browser_textures::live_frame(rig.id), None);
    browser_textures::forget(rig.id);
}

#[test]
fn an_unfocused_window_pauses_on_the_last_frame_and_focus_resumes() {
    let _serial = serial();
    let mut rig = Rig::new("lh-focus", 1.0);
    rig.frame(SHOWN, 1.0, true);
    rig.send_frame(0, 1);
    rig.host.install_frames(&mut rig.backend);
    let texture = rig.texture();
    assert!(!rig.frame(SHOWN, 1.0, false), "nothing to repaint");
    assert_eq!(rig.host.streaming(), 0);
    assert!(
        rig.backend.image_resident(texture),
        "the paused frame stays up"
    );
    rig.frame(SHOWN, 1.0, true);
    assert_eq!(rig.host.streaming(), 1);
    assert_eq!(rig.started.lock().unwrap().len(), 2, "a fresh driver");
    // A zoom across a step reaches the running driver as an update.
    rig.frame(Some([100.0, 75.0]), 1.0, true);
    let update = rig.next_control(1);
    assert!(matches!(
        update,
        Ok(DriverControl::Update { spec, reload: false }) if spec.frame_width == 128
    ));
    browser_textures::forget_live(rig.id);
}

#[test]
fn the_setting_off_drops_the_frame_for_the_snapshot() {
    let _serial = serial();
    let mut rig = Rig::new("lh-off", 1.0);
    rig.frame(SHOWN, 1.0, true);
    rig.send_frame(0, 1);
    rig.host.install_frames(&mut rig.backend);
    rig.state.editor_ui.browser_live.set_live_pages(false);
    assert!(rig.frame(SHOWN, 1.0, true));
    assert_eq!(browser_textures::live_frame(rig.id), None);
    assert!(!rig.backend.image_resident(rig.texture()));
}
