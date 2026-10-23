//! Make a Google Fonts family available: download it once, keep it in the
//! [`FontStore`], register it with `jian-skia`.
//!
//! Registration bumps `jian_skia::font_generation`, which every measure /
//! layout / paint cache compares against, so an open document reflows with
//! the real face on the next frame. Faces are stored with
//! [`FontOrigin::Google`]; startup's `rescan_and_register` brings them back
//! with no network, so a family is downloaded once per machine.
//!
//! Nothing here runs unless someone asked for it: the missing-fonts modal's
//! Download button (worker thread, [`FontDownloads`]) or the MCP
//! `load_fonts` tool (its own connection thread).

use std::sync::mpsc::{self, Receiver, Sender};

use de_editor_core::missing_fonts::FontDownloadStatus;
use de_host_services::google_fonts::{download_family, FontTransport, HttpFontTransport};

use crate::fonts::{FontOrigin, FontStore};
use crate::fonts_error::FontDownloadError;
use crate::google_font_names::{with_identity, FaceIdentity};

/// A family that is available after the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FontDownloadOk {
    /// Downloaded now; `faces` weights / slants stored and registered.
    Downloaded { family: String, faces: usize },
    /// Already in the store from an earlier download; nothing fetched.
    Cached { family: String },
}

/// Ensure `family` is stored and registered, downloading it if this
/// machine has not yet. Blocking — call from a worker thread.
pub(crate) fn ensure_family(
    transport: &dyn FontTransport,
    store: &FontStore,
    family: &str,
) -> Result<FontDownloadOk, FontDownloadError> {
    let family = family.trim();
    if let Some(cached) = store
        .families_from(FontOrigin::Google)
        .into_iter()
        .find(|known| known.eq_ignore_ascii_case(family))
    {
        return Ok(FontDownloadOk::Cached { family: cached });
    }
    let downloaded = download_family(transport, family)?;
    // Rename every face before storing any, so a malformed file leaves
    // nothing half-stored behind.
    let faces = downloaded
        .faces
        .iter()
        .map(|face| {
            with_identity(
                &face.bytes,
                FaceIdentity {
                    family: &downloaded.family,
                    weight: face.weight,
                    italic: face.italic,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = faces.len();
    for face in faces {
        store.import_from(face, FontOrigin::Google)?;
    }
    Ok(FontDownloadOk::Downloaded {
        family: downloaded.family,
        faces: count,
    })
}

/// One finished request, as delivered to the UI loop.
#[derive(Debug)]
pub(crate) struct FontDownloadDone {
    /// The family as requested (the missing-fonts row's spelling).
    pub requested: String,
    pub result: Result<FontDownloadOk, FontDownloadError>,
}

impl crate::DesktopApp {
    /// Start the downloads the user asked for and apply the finished ones:
    /// a downloaded family is registered already, so refreshing the font
    /// snapshots resolves its missing-font row and the next paint reflows
    /// the document with it. Returns whether anything changed.
    pub(crate) fn drain_font_downloads(&mut self) -> bool {
        let requests = self.host.take_font_download_requests();
        let started = !requests.is_empty();
        for family in requests {
            self.font_downloads.spawn(family);
        }
        let done = self.font_downloads.poll();
        if done.is_empty() {
            return started;
        }
        for FontDownloadDone { requested, result } in done {
            let failure = match result {
                Ok(_) => None,
                Err(error) => {
                    eprintln!("[fonts] could not download \"{requested}\": {error}");
                    Some(if error.is_not_on_google() {
                        FontDownloadStatus::NotOnGoogle
                    } else {
                        FontDownloadStatus::Failed
                    })
                }
            };
            self.host.finish_font_download(&requested, failure);
        }
        crate::font_import_host::refresh_downloaded_fonts(&mut self.host);
        true
    }
}

/// Background downloads requested from the UI. One worker thread per
/// family; results arrive on one channel the redraw pump drains.
pub(crate) struct FontDownloads {
    tx: Sender<FontDownloadDone>,
    rx: Receiver<FontDownloadDone>,
    in_flight: usize,
}

impl Default for FontDownloads {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            in_flight: 0,
        }
    }
}

impl FontDownloads {
    /// Start downloading `family` on a worker thread.
    pub(crate) fn spawn(&mut self, family: String) {
        let tx = self.tx.clone();
        self.in_flight += 1;
        // Detached and safe to leave so: every request carries a 20 s
        // timeout and a size cap, and a dropped receiver only turns the
        // final send into a no-op.
        std::thread::spawn(move || {
            let result = FontStore::user()
                .map_err(|e| FontDownloadError::OpenStore(e.to_string()))
                .and_then(|store| ensure_family(&HttpFontTransport, &store, &family));
            let _ = tx.send(FontDownloadDone {
                requested: family,
                result,
            });
        });
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.in_flight > 0
    }

    /// Every download finished since the last call.
    pub(crate) fn poll(&mut self) -> Vec<FontDownloadDone> {
        let done: Vec<FontDownloadDone> = self.rx.try_iter().collect();
        self.in_flight = self.in_flight.saturating_sub(done.len());
        done
    }
}

// Gated off Windows like `fonts.rs`: skia's DirectWrite font manager is not
// safe to drive from cargo's parallel test threads there.
#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    use de_host_services::google_fonts::{css_url, GoogleFontsError, HttpReply};
    use jian_core::layout::measure::{FontStyleKind, MeasureBackend, MeasureRequest, StyledRun};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    // An OFL face that `bundled_fonts::register` never runs for in this test
    // binary, served under a family name no machine has installed.
    const FACE: &[u8] = include_bytes!("../assets/fonts/DMSerifDisplay-Regular.ttf");

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new() -> Self {
            let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("de-google-fonts-test-{}-{seq}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Serves `family` as a one-face family, or fails every request.
    struct Fake {
        family: &'static str,
        offline: bool,
        requests: AtomicUsize,
    }

    const FACE_URL: &str = "https://fonts.gstatic.com/s/fake/v1/regular.ttf";

    impl FontTransport for Fake {
        fn get(&self, url: &str, _max: usize) -> Result<HttpReply, GoogleFontsError> {
            self.requests.fetch_add(1, Ordering::Relaxed);
            if self.offline {
                return Err(GoogleFontsError::Request {
                    url: url.into(),
                    detail: "offline".into(),
                });
            }
            let body = if url == css_url(self.family).unwrap() {
                format!(
                    "@font-face {{ font-family: '{}'; src: url({FACE_URL}) format('truetype'); }}",
                    self.family
                )
                .into_bytes()
            } else if url == FACE_URL {
                FACE.to_vec()
            } else {
                return Ok(HttpReply {
                    status: 400,
                    body: Vec::new(),
                });
            };
            Ok(HttpReply { status: 200, body })
        }
    }

    fn fake(family: &'static str, offline: bool) -> Fake {
        Fake {
            family,
            offline,
            requests: AtomicUsize::new(0),
        }
    }

    fn registered(family: &str) -> bool {
        jian_skia::list_families()
            .iter()
            .any(|meta| meta.family == family)
    }

    fn width(family: &str) -> f32 {
        let runs = [StyledRun {
            text: "Hamburgefonstiv 0123",
            font_family: Some(family),
            font_size: 40.0,
            font_weight: 400,
            font_style: FontStyleKind::Normal,
            letter_spacing: 0.0,
        }];
        jian_skia::SkiaMeasure::new()
            .measure(&MeasureRequest {
                runs: &runs,
                line_height: 48.0,
                max_width: None,
            })
            .width
    }

    #[test]
    fn cache_miss_downloads_renames_stores_and_registers() {
        const FAMILY: &str = "Zz Downloaded Serif";
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        jian_skia::remove_imported_font(FAMILY);
        let transport = fake(FAMILY, false);

        let ok = ensure_family(&transport, &store, FAMILY).expect("download");

        assert_eq!(
            ok,
            FontDownloadOk::Downloaded {
                family: FAMILY.into(),
                faces: 1
            }
        );
        assert!(registered(FAMILY), "the face answers to the requested name");
        assert_eq!(store.families_from(FontOrigin::Google), vec![FAMILY]);
        assert!(store.families_from(FontOrigin::Imported).is_empty());
        jian_skia::remove_imported_font(FAMILY);
    }

    #[test]
    fn cache_hit_fetches_nothing_and_survives_a_restart_offline() {
        const FAMILY: &str = "Zz Cached Serif";
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        ensure_family(&fake(FAMILY, false), &store, FAMILY).expect("first download");

        // A "restart": the live registry is empty, the disk is not.
        jian_skia::remove_imported_font(FAMILY);
        FontStore::at(&root.0).rescan_and_register();
        assert!(registered(FAMILY), "startup rescan restores the family");

        let offline = fake(FAMILY, true);
        let ok = ensure_family(&offline, &store, "zz cached serif").expect("cache hit");
        assert_eq!(
            ok,
            FontDownloadOk::Cached {
                family: FAMILY.into()
            }
        );
        assert_eq!(offline.requests.load(Ordering::Relaxed), 0, "no network");
        jian_skia::remove_imported_font(FAMILY);
    }

    #[test]
    fn failed_download_registers_nothing_and_reports_why() {
        const FAMILY: &str = "Zz Offline Serif";
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);

        let offline = ensure_family(&fake(FAMILY, true), &store, FAMILY).unwrap_err();
        assert!(
            !offline.is_not_on_google(),
            "offline is retryable: {offline}"
        );

        let unknown = ensure_family(&fake(FAMILY, false), &store, "Zz Unknown").unwrap_err();
        assert!(unknown.is_not_on_google(), "{unknown}");

        assert!(!registered(FAMILY));
        assert!(store.families_from(FontOrigin::Google).is_empty());
    }

    #[test]
    fn measurement_uses_the_downloaded_face() {
        const FAMILY: &str = "Zz Measured Serif";
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        jian_skia::remove_imported_font(FAMILY);
        let fallback = width(FAMILY);

        ensure_family(&fake(FAMILY, false), &store, FAMILY).expect("download");

        let real = width(FAMILY);
        assert!(
            (real - fallback).abs() > 1.0,
            "after the download the text measures with the real face \
             (fallback {fallback}, downloaded {real})"
        );
        jian_skia::remove_imported_font(FAMILY);
    }

    #[test]
    fn background_downloads_report_through_the_channel() {
        let mut downloads = FontDownloads::default();
        assert!(!downloads.is_pending());
        downloads
            .tx
            .send(FontDownloadDone {
                requested: "X".into(),
                result: Ok(FontDownloadOk::Cached { family: "X".into() }),
            })
            .unwrap();
        downloads.in_flight = 1;
        let done = downloads.poll();
        assert_eq!(done.len(), 1);
        assert!(!downloads.is_pending());
    }

    /// Live check against Google Fonts: `cargo test -p de-host-desktop
    /// --bin design-editor -- --ignored real_google_fonts`. Montserrat's
    /// served files name themselves "Montserrat Thin"; after the rename
    /// every weight registers as "Montserrat".
    #[test]
    #[ignore = "needs network"]
    fn real_google_fonts_download_registers_every_weight_under_the_family() {
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        let ok = ensure_family(&HttpFontTransport, &store, "Montserrat").expect("download");
        assert_eq!(
            ok,
            FontDownloadOk::Downloaded {
                family: "Montserrat".into(),
                faces: 18
            }
        );
        let faces = jian_skia::list_families()
            .into_iter()
            .find(|meta| meta.family == "Montserrat")
            .map(|meta| meta.face_count);
        assert_eq!(faces, Some(18));
        jian_skia::remove_imported_font("Montserrat");
    }
}
