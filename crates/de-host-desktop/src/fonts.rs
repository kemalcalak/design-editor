//! Native persistence for user-imported and downloaded fonts.
//!
//! User-imported `.ttf` / `.otf` faces, and faces downloaded from Google
//! Fonts on request, live under `<config>/fonts/` next to a small
//! `index.json` that records where each face came from
//! ([`FontOrigin`]). On startup [`FontStore::rescan_and_register`]
//! re-registers every stored face with `jian-skia` so an imported family
//! survives a restart; [`FontStore::import`] validates + copies a new file in
//! and registers it; [`FontStore::remove`] drops a whole family from both disk
//! and the live registry. The in-memory registry + generation-aware
//! invalidation live in `jian-skia`; this module only owns the disk side.

use std::path::{Path, PathBuf};

use de_config_store::ConfigStore;
use jian_core::layout::measure::FontStyleKind;

use crate::fonts_error::FontStoreError;
use serde::{Deserialize, Serialize};

/// Reject absurd font files early. A normal face is well under 1 MiB; a large
/// CJK / variable font can reach a few MiB, so 16 MiB is a generous ceiling
/// that still bounds disk + parse cost and a hostile input.
pub(crate) const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;

const FONTS_SUBDIR: &str = "fonts";
const INDEX_FILE: &str = "fonts/index.json";

/// Serializes every read-modify-write of `index.json` in this process.
/// Google Fonts downloads import from worker threads (the UI's and MCP's)
/// while the UI thread may import or remove, and an unserialized
/// load-modify-save would drop one writer's entries.
static INDEX_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn index_guard() -> std::sync::MutexGuard<'static, ()> {
    // A writer that panicked mid-save leaves at worst a stale index, which
    // the next load tolerates, so a poisoned lock is still usable.
    INDEX_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where a stored face came from. Entries written before downloads existed
/// carry no field and read as [`FontOrigin::Imported`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FontOrigin {
    /// Picked from disk by the user.
    #[default]
    Imported,
    /// Downloaded from Google Fonts (OFL / Apache licensed).
    Google,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FontIndexEntry {
    family: String,
    italic: bool,
    weight: u16,
    hash: u64,
    /// File name within the fonts dir (e.g. `"a1b2c3d4e5f60718.ttf"`).
    file: String,
    #[serde(default)]
    source: FontOrigin,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FontIndex {
    fonts: Vec<FontIndexEntry>,
}

/// Disk-backed store for user-imported fonts.
pub struct FontStore {
    dir: PathBuf,
    index_path: PathBuf,
}

impl FontStore {
    /// The per-user store under the shared Design Editor config dir.
    pub fn user() -> std::io::Result<Self> {
        Ok(Self::at(ConfigStore::user()?.root()))
    }

    /// Store rooted at `config_root/fonts` — used by tests with a temp dir.
    pub fn at(config_root: impl AsRef<Path>) -> Self {
        let root = config_root.as_ref();
        Self {
            dir: root.join(FONTS_SUBDIR),
            index_path: root.join(INDEX_FILE),
        }
    }

    fn load_index(&self) -> FontIndex {
        de_config_store::read_json_path(&self.index_path)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn save_index(&self, index: &FontIndex) -> std::io::Result<()> {
        de_config_store::write_json_path(&self.index_path, index)
    }

    /// Re-register every persisted font with `jian-skia`. A missing or
    /// corrupt entry is dropped from the index (and logged), never fatal — a
    /// bad file must not block startup.
    pub fn rescan_and_register(&self) {
        let _guard = index_guard();
        let index = self.load_index();
        let mut kept: Vec<FontIndexEntry> = Vec::with_capacity(index.fonts.len());
        let mut changed = false;
        for entry in index.fonts {
            let path = self.dir.join(&entry.file);
            match std::fs::read(&path) {
                Ok(bytes) => match jian_skia::register_imported_font(bytes) {
                    Ok(_) => kept.push(entry),
                    Err(err) => {
                        eprintln!(
                            "[fonts] dropping unparseable imported font {}: {err}",
                            path.display()
                        );
                        let _ = std::fs::remove_file(&path);
                        changed = true;
                    }
                },
                Err(err) => {
                    eprintln!(
                        "[fonts] dropping missing imported font {}: {err}",
                        path.display()
                    );
                    changed = true;
                }
            }
        }
        if changed {
            if let Err(err) = self.save_index(&FontIndex { fonts: kept }) {
                eprintln!("[fonts] failed to rewrite font index: {err}");
            }
        }
    }

    /// Validate, persist, then register a font file. Returns the registered
    /// blob on success. Rejects oversize / unparseable input without touching
    /// disk.
    ///
    /// Persistence happens BEFORE the live registration: the bytes are parsed
    /// (no registry mutation) to learn the file/index key, written to disk,
    /// and only then committed to the process-global registry. Registering
    /// first would leave a live-but-unpersisted font — visible + rendered
    /// this session but gone on restart — if a disk write then failed while
    /// the caller reported the import as failed.
    pub fn import(&self, bytes: Vec<u8>) -> Result<jian_skia::FontBlob, FontStoreError> {
        self.import_from(bytes, FontOrigin::Imported)
    }

    /// [`Self::import`], recording `origin` in the index.
    pub(crate) fn import_from(
        &self,
        bytes: Vec<u8>,
        origin: FontOrigin,
    ) -> Result<jian_skia::FontBlob, FontStoreError> {
        let mut blobs = self.import_all_from(vec![bytes], origin)?;
        blobs.pop().ok_or(FontStoreError::NotAFontFile)
    }

    /// Store several faces as one transaction: every face is validated
    /// before any file is written, and one index save commits them all, so
    /// a failure leaves none of them indexed (a downloaded family is never
    /// half stored). Returns the registered blobs in input order.
    pub(crate) fn import_all_from(
        &self,
        faces: Vec<Vec<u8>>,
        origin: FontOrigin,
    ) -> Result<Vec<jian_skia::FontBlob>, FontStoreError> {
        let _guard = index_guard();
        // Parse WITHOUT registering — validates the bytes and yields the
        // family/style/weight/hash the on-disk entry is keyed on.
        let mut parsed = Vec::with_capacity(faces.len());
        for bytes in faces {
            if bytes.len() > MAX_FONT_BYTES {
                return Err(FontStoreError::TooLarge { bytes: bytes.len() });
            }
            let meta =
                jian_skia::parse_imported_font_meta(&bytes).ok_or(FontStoreError::NotAFontFile)?;
            parsed.push((meta, bytes));
        }

        // `std::io::Error` and `jian-skia`'s error belong to code this pass
        // does not own, so their messages ride along as text.
        std::fs::create_dir_all(&self.dir).map_err(|e| FontStoreError::CreateDir(e.to_string()))?;
        let mut written: Vec<String> = Vec::with_capacity(parsed.len());
        // Files this call created; a failure removes only these, never a
        // same-named file an existing entry already points at.
        let mut created: Vec<String> = Vec::new();
        for (meta, bytes) in &parsed {
            let file = format!("{:016x}.ttf", meta.hash);
            let path = self.dir.join(&file);
            let existed = path.exists();
            if let Err(e) = std::fs::write(&path, bytes) {
                self.remove_files(&created);
                return Err(FontStoreError::WriteFile(e.to_string()));
            }
            if !existed {
                created.push(file.clone());
            }
            written.push(file);
        }

        let mut index = self.load_index();
        // Replace any prior entry for the same face (last-import-wins, matching
        // the in-memory registry). COLLECT the superseded files but do NOT
        // delete them yet: deleting before the new index is durably saved
        // would lose the previous font if `save_index` then failed, leaving
        // the on-disk index pointing at a file we already removed.
        let mut superseded: Vec<String> = Vec::new();
        for ((meta, _), file) in parsed.iter().zip(&written) {
            let italic = matches!(meta.style, FontStyleKind::Italic);
            index.fonts.retain(|e| {
                let same = e.family == meta.family && e.italic == italic && e.weight == meta.weight;
                if same && &e.file != file {
                    superseded.push(e.file.clone());
                }
                !same
            });
            index.fonts.push(FontIndexEntry {
                family: meta.family.clone(),
                italic,
                weight: meta.weight,
                hash: meta.hash,
                file: file.clone(),
                source: origin,
            });
        }
        if let Err(e) = self.save_index(&index) {
            self.remove_files(&created);
            return Err(FontStoreError::WriteIndex(e.to_string()));
        }

        // The new index is durable — only now is it safe to prune the files
        // it no longer references.
        superseded.retain(|old| !written.contains(old));
        self.remove_files(&superseded);

        // Persistence is durable — now commit the live registration. This
        // re-parses the same bytes, so it cannot fail after `parse_*` above
        // succeeded, but propagate any error rather than unwrap.
        parsed
            .into_iter()
            .map(|(_, bytes)| {
                jian_skia::register_imported_font(bytes)
                    .map_err(|e| FontStoreError::Register(e.to_string()))
            })
            .collect()
    }

    fn remove_files(&self, files: &[String]) {
        for file in files {
            let _ = std::fs::remove_file(self.dir.join(file));
        }
    }

    /// Register `family`'s stored faces when the live registry lacks them —
    /// another process (a headless `--mcp`, a second instance) stored the
    /// family after this one's startup rescan. Returns whether the family
    /// is registered afterwards.
    pub(crate) fn ensure_registered(&self, family: &str) -> bool {
        let registered = || {
            jian_skia::list_families()
                .iter()
                .any(|meta| meta.family.eq_ignore_ascii_case(family))
        };
        if registered() {
            return true;
        }
        let _guard = index_guard();
        for entry in self
            .load_index()
            .fonts
            .iter()
            .filter(|entry| entry.family.eq_ignore_ascii_case(family))
        {
            if let Ok(bytes) = std::fs::read(self.dir.join(&entry.file)) {
                let _ = jian_skia::register_imported_font(bytes);
            }
        }
        registered()
    }

    /// Families with at least one stored face from `origin`, sorted and
    /// deduplicated case-insensitively.
    pub(crate) fn families_from(&self, origin: FontOrigin) -> Vec<String> {
        let mut families: Vec<String> = self
            .load_index()
            .fonts
            .into_iter()
            .filter(|entry| entry.source == origin)
            .map(|entry| entry.family)
            .collect();
        families.sort_by_key(|family| family.to_ascii_lowercase());
        families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        families
    }

    /// Remove every imported face of `family` from the live registry and disk.
    /// Returns whether anything was removed.
    pub fn remove(&self, family: &str) -> bool {
        let _guard = index_guard();
        let removed_live = jian_skia::remove_imported_font(family);
        let mut index = self.load_index();
        let before = index.fonts.len();
        index.fonts.retain(|e| {
            if e.family == family {
                let _ = std::fs::remove_file(self.dir.join(&e.file));
                false
            } else {
                true
            }
        });
        let removed_disk = index.fonts.len() != before;
        if removed_disk {
            let _ = self.save_index(&index);
        }
        removed_live || removed_disk
    }
}

// Gated off Windows: these register fonts through skia's `FontMgr`
// (`register_imported_font` → `FontMgr::new_from_data`, the DirectWrite
// backend on Windows) from cargo's parallel test-worker threads. That
// combination segfaults (STATUS_ACCESS_VIOLATION) in Windows CI. Production
// touches fonts only from the main render thread, and macOS + Linux keep the
// coverage. (Runs single-threaded/serially would also avoid it, but a cfg
// gate is the least invasive.)
#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // A bundled family distinct from those used by de-host-native's font
    // tests (different test binary anyway) and unlikely to be system-installed.
    const FONT: &[u8] = include_bytes!("../assets/fonts/InstrumentSerif-Regular.ttf");
    const FAMILY: &str = "Instrument Serif";

    // A second distinct family for the failed-persist test, so it can't race
    // the round-trip test's family in the shared process-global registry.
    const FONT2: &[u8] = include_bytes!("../assets/fonts/Outfit-VF.ttf");
    const FAMILY2: &str = "Outfit";

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new() -> Self {
            let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("de-fonts-test-{}-{seq}", std::process::id()));
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

    #[test]
    fn import_persists_registers_then_rescan_and_remove_round_trip() {
        let root = TempRoot::new();
        // Start clean in the process-global registry too.
        jian_skia::remove_imported_font(FAMILY);

        let store = FontStore::at(&root.0);

        // --- import: validates, registers, writes the file + index ---------
        let blob = store.import(FONT.to_vec()).expect("import a valid font");
        assert_eq!(blob.family, FAMILY);
        let stored_file = root
            .0
            .join(FONTS_SUBDIR)
            .join(format!("{:016x}.ttf", blob.hash));
        assert!(stored_file.exists(), "font file copied into the store");
        assert!(root.0.join(INDEX_FILE).exists(), "index written");
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY)
                .count(),
            1,
            "family registered in the live registry"
        );

        // --- rescan: a fresh store over the same dir re-registers ----------
        jian_skia::remove_imported_font(FAMILY);
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY)
                .count(),
            0,
            "cleared before rescan"
        );
        FontStore::at(&root.0).rescan_and_register();
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY)
                .count(),
            1,
            "rescan re-registers the persisted family"
        );

        // --- remove: drops the family from disk + registry -----------------
        assert!(store.remove(FAMILY), "remove reports a change");
        assert!(!stored_file.exists(), "font file deleted");
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY)
                .count(),
            0,
            "family gone from the live registry"
        );
        // Index no longer lists it, so a rescan is a no-op.
        FontStore::at(&root.0).rescan_and_register();
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY)
                .count(),
            0,
            "nothing to re-register after removal"
        );
    }

    #[test]
    fn oversize_and_corrupt_imports_are_rejected_without_touching_disk() {
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);

        let too_big = vec![0u8; MAX_FONT_BYTES + 1];
        assert!(store.import(too_big).is_err(), "oversize rejected");

        let not_a_font = b"this is definitely not a font file".to_vec();
        assert!(store.import(not_a_font).is_err(), "corrupt bytes rejected");

        // Neither wrote an index (no successful import happened).
        assert!(
            !root.0.join(INDEX_FILE).exists(),
            "a rejected import must not create the index"
        );
    }

    #[test]
    fn failed_persist_does_not_leak_into_the_live_registry() {
        // Root is a FILE, not a directory, so `create_dir_all(root/fonts)`
        // fails and the import errors AFTER parse but BEFORE the live
        // registration — proving persistence precedes registration.
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_root =
            std::env::temp_dir().join(format!("de-fonts-notadir-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&file_root);
        std::fs::write(&file_root, b"x").unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(file_root.clone());

        jian_skia::remove_imported_font(FAMILY2); // clean slate in the shared registry

        let store = FontStore::at(&file_root);
        assert!(
            store.import(FONT2.to_vec()).is_err(),
            "import into a file-rooted store must fail at the disk write"
        );
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY2)
                .count(),
            0,
            "a failed persist must NOT leave the font registered in the live registry"
        );
    }

    #[test]
    fn replacement_import_prunes_the_old_file_only_after_saving_the_index() {
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        jian_skia::remove_imported_font(FAMILY2);

        // Learn FONT2's real face key so the seeded "old" entry collides on
        // (family, style, weight) and is treated as the replacement target.
        let meta = jian_skia::parse_imported_font_meta(FONT2).expect("FONT2 parses");
        let italic = matches!(meta.style, FontStyleKind::Italic);

        // Seed a pre-existing persisted face pointing at a DIFFERENT file.
        let fonts_dir = root.0.join(FONTS_SUBDIR);
        std::fs::create_dir_all(&fonts_dir).unwrap();
        let old_file = "oldface.ttf";
        std::fs::write(fonts_dir.join(old_file), b"stale").unwrap();
        store
            .save_index(&FontIndex {
                fonts: vec![FontIndexEntry {
                    family: meta.family.clone(),
                    italic,
                    weight: meta.weight,
                    hash: 1,
                    file: old_file.to_string(),
                    source: FontOrigin::Imported,
                }],
            })
            .unwrap();

        // Import the real font — a replacement of that face.
        let blob = store.import(FONT2.to_vec()).expect("replacement import");

        assert!(
            !fonts_dir.join(old_file).exists(),
            "superseded file pruned after the index was saved"
        );
        assert!(
            fonts_dir.join(format!("{:016x}.ttf", blob.hash)).exists(),
            "new face file written"
        );
        assert_eq!(store.load_index().fonts.len(), 1, "index holds one face");
        assert_eq!(
            jian_skia::list_families()
                .iter()
                .filter(|m| m.family == FAMILY2)
                .count(),
            1,
            "exactly one live face for the replaced family"
        );

        jian_skia::remove_imported_font(FAMILY2);
    }

    #[test]
    fn legacy_index_entries_read_as_imported() {
        let root = TempRoot::new();
        let fonts_dir = root.0.join(FONTS_SUBDIR);
        std::fs::create_dir_all(&fonts_dir).unwrap();
        std::fs::write(
            root.0.join(INDEX_FILE),
            r#"{"fonts":[{"family":"Old Face","italic":false,"weight":400,"hash":7,"file":"x.ttf"}]}"#,
        )
        .unwrap();
        let store = FontStore::at(&root.0);
        assert_eq!(store.families_from(FontOrigin::Imported), vec!["Old Face"]);
        assert!(store.families_from(FontOrigin::Google).is_empty());
    }

    #[test]
    fn a_batch_with_one_bad_face_stores_none_of_them() {
        let root = TempRoot::new();
        let store = FontStore::at(&root.0);
        let result = store.import_all_from(
            vec![FONT2.to_vec(), b"not a font".to_vec()],
            FontOrigin::Google,
        );
        assert!(matches!(result, Err(FontStoreError::NotAFontFile)));
        assert!(!root.0.join(INDEX_FILE).exists(), "nothing indexed");
        assert!(
            !root.0.join(FONTS_SUBDIR).exists()
                || std::fs::read_dir(root.0.join(FONTS_SUBDIR))
                    .unwrap()
                    .next()
                    .is_none(),
            "no face file left behind"
        );
    }
}
