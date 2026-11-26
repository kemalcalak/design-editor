//! FNV-1a 64, for names that must stay stable across runs and builds
//! (content-addressed file names). `std`'s hashers are seeded per process.

/// FNV-1a 64 of `bytes`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_extend(0xcbf2_9ce4_8422_2325, bytes)
}

/// Continue an FNV-1a 64 hash over more bytes, so several fields hash as
/// one stream without being concatenated first.
pub fn fnv1a64_extend(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_the_reference_vectors() {
        assert_eq!(super::fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn extending_hashes_one_stream() {
        assert_eq!(
            super::fnv1a64_extend(super::fnv1a64(b"ab"), b"cd"),
            super::fnv1a64(b"abcd")
        );
    }
}
