//! Stable names for Home's Recent thumbnails. The desktop writes a small
//! PNG of a document's active page after each save; Home looks it up by
//! the same names, so the writer and the reader never disagree.

/// FNV-1a over `bytes` — stable across runs and platforms (unlike
/// `std`'s randomly seeded hasher), which a file name needs.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The thumbnail file for the document at `path`, relative to the
/// thumbnails folder. One file per document; a new save overwrites it.
pub fn home_thumbnail_file_name(path: &str) -> String {
    format!("{:016x}.png", fnv1a(path.as_bytes()))
}

/// The image-cache id a thumbnail is installed under. It folds in the
/// document's last-touched time, so a re-saved document gets a fresh id
/// and Home never shows the previous version's cached pixels.
pub fn home_thumbnail_image_id(path: &str, modified_at: u64) -> u64 {
    let mut key = Vec::with_capacity(path.len() + 9);
    key.extend_from_slice(b"thumb:");
    key.extend_from_slice(path.as_bytes());
    key.extend_from_slice(&modified_at.to_le_bytes());
    fnv1a(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_distinct_per_document() {
        let a = home_thumbnail_file_name("/Users/me/a.op");
        assert_eq!(a, home_thumbnail_file_name("/Users/me/a.op"));
        assert_ne!(a, home_thumbnail_file_name("/Users/me/b.op"));
        assert!(a.ends_with(".png") && a.len() == 20);
    }

    #[test]
    fn a_new_save_gets_a_new_image_id() {
        let first = home_thumbnail_image_id("/a.op", 10);
        assert_eq!(first, home_thumbnail_image_id("/a.op", 10));
        assert_ne!(first, home_thumbnail_image_id("/a.op", 11));
        assert_ne!(first, home_thumbnail_image_id("/b.op", 10));
    }
}
