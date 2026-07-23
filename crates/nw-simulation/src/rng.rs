//! Deterministic randomness: an in-crate PCG-XSH-RR 64/32 generator and the
//! three named streams the simulation contract requires. Each stream derives
//! from the scenario seed plus a stable identifier, so systems cannot share
//! streams and adding a draw to one system cannot perturb another. In-crate so
//! no dependency upgrade can silently change replays.

use nw_content::hash::fnv1a64;
use serde::{Deserialize, Serialize};

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// PCG-XSH-RR 64/32 with the reference constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Derive a stream from the scenario seed and a stable stream identifier.
    pub fn for_stream(seed: u64, stream: &str) -> Pcg32 {
        let stream_id = fnv1a64(stream.as_bytes());
        let mut rng = Pcg32 {
            state: splitmix64(seed ^ stream_id),
            inc: (stream_id << 1) | 1,
        };
        // One warm-up step decorrelates near-identical seeds.
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform draw in `lo..=hi`.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi - lo + 1) as u64;
        lo + (u64::from(self.next_u32()) % span) as i64
    }

    /// Uniform draw in `lo..=hi`.
    pub fn range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        debug_assert!(lo <= hi);
        lo + u64::from(self.next_u32()) % (hi - lo + 1)
    }

    /// Uniform index into a slice of `len` elements.
    pub fn pick(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        self.next_u32() as usize % len
    }
}

/// The three named streams. Systems may not share streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}
