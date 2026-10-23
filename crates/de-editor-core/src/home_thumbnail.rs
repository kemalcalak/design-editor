//! Stable names for Home's Recent thumbnails. The hosts write a small PNG
//! overview of a document's active page whenever it is saved; Home looks it
//! up by the same names, so the writer and the reader never disagree.

/// Names must be stable across runs and platforms (unlike `std`'s
/// randomly seeded hasher), so they hash with the crate's FNV-1a.
use crate::web_sync::fnv1a64 as fnv1a;

/// The thumbnail file for the document at `path`, relative to the
/// thumbnails folder. One file per document; a new save overwrites it.
pub fn home_thumbnail_file_name(path: &str) -> String {
    format!("{:016x}.png", fnv1a(path.as_bytes()))
}

/// The image-cache id a thumbnail is installed under. It folds in the
/// preview's stamp (`HomeState::preview_stamps`), so a refreshed preview
/// gets a fresh id and Home never shows the previous version's pixels.
pub fn home_thumbnail_image_id(path: &str, stamp: u64) -> u64 {
    const PREFIX: &[u8] = b"thumb:";
    let mut key = Vec::with_capacity(PREFIX.len() + path.len() + size_of::<u64>());
    key.extend_from_slice(PREFIX);
    key.extend_from_slice(path.as_bytes());
    key.extend_from_slice(&stamp.to_le_bytes());
    fnv1a(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_distinct_per_document() {
        let a = home_thumbnail_file_name("/Users/me/a.de");
        assert_eq!(a, home_thumbnail_file_name("/Users/me/a.de"));
        assert_ne!(a, home_thumbnail_file_name("/Users/me/b.de"));
        assert!(a.ends_with(".png") && a.len() == 20);
    }

    #[test]
    fn a_new_preview_gets_a_new_image_id() {
        let first = home_thumbnail_image_id("/a.de", 10);
        assert_eq!(first, home_thumbnail_image_id("/a.de", 10));
        assert_ne!(first, home_thumbnail_image_id("/a.de", 11));
        assert_ne!(first, home_thumbnail_image_id("/b.de", 10));
    }
}
