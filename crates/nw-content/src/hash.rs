//! FNV-1a 64 — the workspace's stable, dependency-free hash. Used for content
//! versions, RNG stream identifiers, and (via nw-persistence) state digests.
//! In-crate so no dependency upgrade can silently change replays.

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_vector() {
        // The canonical FNV-1a 64 test vector.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
