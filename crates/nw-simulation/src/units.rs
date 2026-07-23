//! Canonical simulation units. All authoritative quantities are integer or
//! fixed-point so results are identical across platforms.

use serde::{Deserialize, Serialize};

/// A canonical integer simulation tick. Every duration and accrual derives from
/// ticks. Ranked time is the count of active, unpaused ticks from start to a
/// valid victory.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Tick(pub u64);

impl Tick {
    /// The tick before any command has been applied.
    pub const START: Tick = Tick(0);

    /// The next tick.
    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }
}

/// Gross greenhouse-gas emissions, stored as milli-GtCO2e/year (integer
/// thousandths of a gigatonne) so the simulation never rounds through floats.
/// Displayed as simplified, rounded GtCO2e/year. While carbon removal is
/// absent, this is "gross" / "global" emissions, never "net".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct MilliGt(pub i64);

impl MilliGt {
    /// Global gross emissions of zero is the victory threshold for this quantity.
    pub const ZERO: MilliGt = MilliGt(0);
}

/// Abstract global CFA finance units. One global stock with no maximum; each
/// continent's contribution is visible. A modest positive baseline income
/// guarantees recoverability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Finance(pub i64);

/// Abstract global mandate points: CFA treaty coordination authority, not local
/// public consent. Tracked with a current value, a maximum, and an income rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Mandate(pub i64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_advance_by_one() {
        assert_eq!(Tick::START.next(), Tick(1));
    }

    #[test]
    fn gross_emissions_zero_is_the_victory_value() {
        assert_eq!(MilliGt::ZERO, MilliGt(0));
    }
}
