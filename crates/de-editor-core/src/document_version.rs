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
    pub fn from_opened_document(mut doc: PenDocument) -> Self {
        stamp_product_version(&mut doc);
        crate::component_set::normalize_set_refs(&mut doc);
        Self::from_document(doc)
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

    #[test]
    fn stamping_is_idempotent() {
        let mut d = doc(r#"{"version":"0.5.5","children":[]}"#);
        stamp_product_version(&mut d);
        let once = serde_json::to_string(&d).expect("serializes");
        stamp_product_version(&mut d);
        assert_eq!(serde_json::to_string(&d).expect("serializes"), once);
    }
}
