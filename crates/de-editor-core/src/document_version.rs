//! What the two version fields of a `.de` file mean (#408).
//!
//! - `version` is the Design Editor release that last wrote the file. A new
//!   document is born with it ([`EditorState::starter`](crate::EditorState::starter)
//!   and [`EditorState::new`](crate::EditorState::new)), and a document the
//!   editor opens or imports goes through
//!   [`EditorState::from_opened_document`], which re-stamps it with
//!   [`stamp_product_version`], so the next save records the release that
//!   saved it (except Save As of an unedited document, which copies the
//!   file's bytes and rewrites only `editorMeta`).
//! - `formatVersion` is the schema version (`jian_ops_schema::version`),
//!   and it is the field readers check. Files without it are read as the
//!   legacy schema, where older tools (the retired TS editor, Jian apps)
//!   used `version` itself as the schema marker, and the loaders still fall
//!   back to it there (the legacy-major retry, the responsive-minor warning).
//!   This editor writes no `formatVersion`, so its own 0.x releases read as
//!   the legacy v0 schema: harmless while the product is 0.x (see
//!   `product_is_still_0x` below before releasing 1.0).

use crate::EditorState;
use jian_ops_schema::PenDocument;

/// The Design Editor release this build is (the workspace version).
/// `EditorState::new` / `starter` spell `env!("CARGO_PKG_VERSION")` out
/// instead, because `tools/check-version-sync.sh` checks those sites literally.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Record this build as the release that writes `doc` from now on.
///
/// A file without `formatVersion` whose `version` is a 1.x schema marker
/// (a Jian app or TS editor file) keeps that marker: it moves to
/// `formatVersion` as `major.minor` first, so readers see the same schema
/// version as before (capped at the newest schema this build knows: an
/// unknown 1.x minor was read without a warning, and an explicit one would
/// raise a future-version warning that blocks rewrites). A 0.x `version` is this product's own release (or a
/// legacy v0 file) and is simply replaced.
///
/// A freshly parsed document's pending `imageThumbs` seed
/// (`jian_ops_schema::image_thumbs::attach_to_document`) is keyed on the
/// `version` buffer this replaces, so stamping a parsed document that has
/// not been activated yet loses its thumbnails; open documents through
/// [`EditorState::from_opened_document`], which takes the seed first.
pub fn stamp_product_version(doc: &mut PenDocument) {
    if doc.version == PRODUCT_VERSION {
        return;
    }
    if doc.format_version.is_none() {
        let declared = Some(doc.version.as_str());
        let (major, minor) = jian_ops_schema::version::parse(declared);
        if major >= 1 && jian_ops_schema::version::supports(declared) {
            let current = jian_ops_schema::version::parse(Some(
                jian_ops_schema::version::FORMAT_VERSION_CURRENT,
            ));
            let (major, minor) = (major, minor).min(current);
            doc.format_version = Some(format!("{major}.{minor}"));
        }
    }
    doc.version = PRODUCT_VERSION.to_owned();
}

impl EditorState {
    /// Editor state for a document opened from a file or an import: like
    /// [`EditorState::from_document`], with `version` re-stamped so whatever
    /// writes it next records this release. The stamp is not an edit: the
    /// document opens clean.
    /// Instances whose `ref` names a component set (pen.dev's form) are
    /// pointed at the variant master they show (#368).
    ///
    /// The document's `imageThumbs` table is taken into the state's own
    /// table before the stamp: the schema keys a parsed document's pending
    /// thumbnail seed on its `version` buffer, which the stamp replaces, so
    /// taking it afterwards (in `from_document`) found nothing for every file
    /// this release did not write: legacy-schema files and older releases'
    /// alike (#458).
    pub fn from_opened_document(mut doc: PenDocument) -> Self {
        let thumbs = jian_ops_schema::image_thumbs::ImageThumbs::take_for_document(&doc);
        stamp_product_version(&mut doc);
        crate::component_set::normalize_set_refs(&mut doc);
        let mut state = Self::from_document(doc);
        if let Some(thumbs) = thumbs {
            state.image_thumbs = thumbs;
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::{stamp_product_version, PRODUCT_VERSION};

    fn doc(source: &str) -> jian_ops_schema::PenDocument {
        jian_ops_schema::load_str(source)
            .expect("fixture parses")
            .value
    }

    #[test]
    fn an_older_release_is_replaced_by_this_one() {
        let mut d = doc(r#"{"version":"0.5.5","children":[]}"#);
        stamp_product_version(&mut d);
        assert_eq!(d.version, PRODUCT_VERSION);
        assert_eq!(d.format_version, None);
    }

    #[test]
    fn a_declared_format_version_is_left_alone() {
        let mut d = doc(r#"{"version":"1.1","formatVersion":"1.2","children":[]}"#);
        stamp_product_version(&mut d);
        assert_eq!(d.version, PRODUCT_VERSION);
        assert_eq!(d.format_version.as_deref(), Some("1.2"));
    }

    #[test]
    fn a_legacy_schema_marker_moves_to_format_version() {
        let mut d = doc(r#"{"version":"1.2.0","children":[]}"#);
        stamp_product_version(&mut d);
        assert_eq!(d.version, PRODUCT_VERSION);
        assert_eq!(d.format_version.as_deref(), Some("1.2"));
    }

    /// Tripwire for the 1.0 release: from 1.0 on, a file this editor wrote
    /// (no `formatVersion`) carries a 1.x `version` that
    /// `stamp_product_version` would take for a schema marker and copy into
    /// `formatVersion`. Before bumping to 1.0, make the editor write
    /// `formatVersion` (or drop the carry-over) and update this test.
    #[test]
    fn product_is_still_0x() {
        assert!(PRODUCT_VERSION.starts_with("0."), "{PRODUCT_VERSION}");
    }

    #[test]
    fn a_legacy_marker_newer_than_the_schema_is_capped() {
        let mut d = doc(r#"{"version":"1.9","children":[]}"#);
        stamp_product_version(&mut d);
        assert_eq!(
            d.format_version.as_deref(),
            Some(jian_ops_schema::version::FORMAT_VERSION_CURRENT)
        );
    }

    /// Opening registers a file's `imageThumbs` whatever its version fields
    /// say (#458): only a file this release wrote kept them before, because
    /// the stamp replaced the `version` buffer their seed was keyed on.
    #[test]
    fn opening_registers_thumbnails_for_every_version_shape() {
        // A minimal JPEG (SOI, EOI) and its base64.
        let jpeg: &[u8] = &[0xff, 0xd8, 0xff, 0xd9];
        let current = format!(r#""version":"{PRODUCT_VERSION}","formatVersion":"1.0""#);
        let cases = [
            ("legacy 1.x schema marker", r#""version":"1.0.0""#),
            ("legacy 0.x schema marker", r#""version":"0.8.0""#),
            (
                "older release, current schema",
                r#""version":"0.5.5","formatVersion":"1.0""#,
            ),
            ("this release", current.as_str()),
        ];
        for (index, (label, versions)) in cases.iter().enumerate() {
            let paint_id = 9_458_000 + index as u64;
            let source = format!(
                r#"{{{versions},"imageThumbs":{{"{paint_id}":"/9j/2Q=="}},"children":[]}}"#
            );
            let state = crate::EditorState::from_opened_document(doc(&source));
            assert_eq!(state.doc.version, PRODUCT_VERSION, "{label}");
            assert_eq!(
                state.image_thumbs.get(paint_id).as_deref(),
                Some(jpeg),
                "{label}: thumbnail registered on open"
            );
        }
    }

    #[test]
    fn stamping_is_idempotent() {
        let mut d = doc(r#"{"version":"0.5.5","children":[]}"#);
        stamp_product_version(&mut d);
        let once = serde_json::to_string(&d).expect("serializes");
        stamp_product_version(&mut d);
        assert_eq!(serde_json::to_string(&d).expect("serializes"), once);
    }
}
