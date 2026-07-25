//! Deterministic randomness: the fleet's PCG-XSH-RR 64/32 generator and the
//! three named streams the simulation contract requires. Each stream derives
//! from the scenario seed plus a stable identifier, so systems cannot share
//! streams and adding a draw to one system cannot perturb another.
//!
//! The generator is `vellum-rng`'s unified construction — `Pcg32::seeded`
//! with this crate's stream names hashed into stream selectors, and the
//! Lemire bounded draw underneath every helper. This replaced the in-crate
//! generator (a different seed mix, and modulo draws that carried a slight
//! bias) under the fleet decision `rng-unification-breaks-saves`; this game
//! had no pinned run fixtures, so nothing needed re-blessing — the seeded
//! bots simply had to keep winning, and they do.

use nw_content::hash::fnv1a64;
use serde::{Deserialize, Serialize};

/// The simulation's PCG32, stored as the fleet's shared generator type.
///
/// A thin vocabulary wrapper: the type, seeding, and draws are `vellum-rng`'s;
/// the helper names (`range_i64`, `range_u64`, `pick`) are this crate's.
/// `serde(transparent)` keeps the serialised shape exactly the inner
/// `{ state, inc }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pcg32 {
    inner: vellum_rng::Pcg32,
}

impl Pcg32 {
    /// Derive a stream from the scenario seed and a stable stream identifier.
    pub fn for_stream(seed: u64, stream: &str) -> Pcg32 {
        Pcg32 {
            inner: vellum_rng::Pcg32::seeded(seed, fnv1a64(stream.as_bytes())),
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    /// Uniform draw in `lo..=hi`. Spans stay within `u32`, which every caller
    /// respects by construction (game quantities, not raw 64-bit ranges).
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = u64::try_from(hi - lo).expect("range_i64 span is non-negative") + 1;
        let span = u32::try_from(span).expect("range_i64 spans stay within u32");
        lo + i64::from(self.inner.below(span))
    }

    /// Uniform draw in `lo..=hi`. Spans stay within `u32`.
    pub fn range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        debug_assert!(lo <= hi);
        let span = u32::try_from(hi - lo + 1).expect("range_u64 spans stay within u32");
        lo + u64::from(self.inner.below(span))
    }

    /// Uniform index into a slice of `len` elements.
    pub fn pick(&mut self, len: usize) -> usize {
        self.inner.pick_index(len)
    }
}

/// The three named streams. Systems may not share streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngStreams {
    pub starting_variation: Pcg32,
    pub opportunity_timing: Pcg32,
    pub opportunity_selection: Pcg32,
}

impl RngStreams {
    pub fn new(seed: u64) -> RngStreams {
        RngStreams {
            starting_variation: Pcg32::for_stream(seed, "starting_variation"),
            opportunity_timing: Pcg32::for_stream(seed, "opportunity_timing"),
            opportunity_selection: Pcg32::for_stream(seed, "opportunity_selection"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_isolated() {
        let mut a = RngStreams::new(99);
        let mut b = RngStreams::new(99);
        // Extra draws on one stream must not perturb another.
        for _ in 0..17 {
            a.opportunity_selection.next_u32();
        }
        assert_eq!(
            a.starting_variation.next_u32(),
            b.starting_variation.next_u32()
        );
        assert_eq!(
            a.opportunity_timing.next_u32(),
            b.opportunity_timing.next_u32()
        );
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Pcg32::for_stream(7, "starting_variation");
        let mut b = Pcg32::for_stream(7, "starting_variation");
        let left: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let right: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(left, right);
    }

    #[test]
    fn different_streams_differ() {
        let mut a = Pcg32::for_stream(7, "starting_variation");
        let mut b = Pcg32::for_stream(7, "opportunity_timing");
        assert_ne!(
            (0..4).map(|_| a.next_u32()).collect::<Vec<_>>(),
            (0..4).map(|_| b.next_u32()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn draws_stay_in_their_ranges() {
        let mut rng = Pcg32::for_stream(3, "starting_variation");
        for _ in 0..500 {
            let v = rng.range_i64(-4, 9);
            assert!((-4..=9).contains(&v));
            let u = rng.range_u64(10, 12);
            assert!((10..=12).contains(&u));
            assert!(rng.pick(5) < 5);
        }
    }
}
