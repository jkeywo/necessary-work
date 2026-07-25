//! FNV-1a 64 — the workspace's stable, dependency-free hash. Used for content
//! versions, RNG stream identifiers, and (via nw-persistence) state digests.
//!
//! The implementation is the fleet's, from `vellum-digest` — byte-identical to
//! the in-crate original (same offset basis, same prime), so adopting it moved
//! no value anywhere. The local name stays: `fnv1a64` is this workspace's
//! vocabulary.

pub use vellum_digest::fnv1a as fnv1a64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_vector() {
        // The canonical FNV-1a 64 test vector — also proof the shared
        // implementation is the same function the in-crate one was.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
