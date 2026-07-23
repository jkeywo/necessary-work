//! The authoritative run state. The simulation alone owns it; clients read it
//! and issue semantic commands. All quantities are integers (milli-units), so
//! native and WASM builds cannot diverge.
//!
//! Derived fields (emissions, icons, deltas, slots) are recomputed from the
//! standing completed/active project lists every tick, which is what makes
//! cancellation and decommission cleanup exact by construction: remove the
//! entry and the next derivation has never heard of it.

use nw_content::schema::{EffectOp, Scope};
use nw_content::world::Continent;
use serde::{Deserialize, Serialize};

use crate::rng::RngStreams;
use crate::trace::{BlockReason, TraceEvent};

/// A queued project: no resources reserved, no slot occupied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedProject {
    pub project: u32,
    pub lead: Continent,
}

/// An active build: full costs paid upfront, one slot occupied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveProject {
    pub project: u32,
    pub lead: Continent,
    pub started: u64,
    pub progress: u64,
    /// Duration after modifiers, locked at start.
    pub duration: u64,
}

/// One effect as realised at completion: reductions carry their scaled
/// magnitude, locked in with the icons held at completion time. This is what
/// makes a preview equal the realised effect under unchanged conditions, and
/// keeps completed effects from retroactively growing with later icons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizedEffect {
    pub op: EffectOp,
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedProject {
    pub project: u32,
    pub lead: Continent,
    pub tick: u64,
    pub realized: Vec<RealizedEffect>,
    pub bonus_permille: i64,
}

/// A spawned, unclaimed opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenOpportunity {
    pub def: u32,
    pub continent: Continent,
    pub opened: u64,
    pub expires: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    // Identity and clock. `tick` counts active, unpaused simulation ticks —
    // it does not advance while paused, so it is also the ranked time.
    pub tick: u64,
    pub paused: bool,
    pub seed: u64,
    pub streams: RngStreams,

    // Authored-with-variation starting conditions.
    pub baseline_emissions_milli: [[i64; 3]; 3],
    pub baseline_finance_income_milli: [i64; 3],
    pub baseline_mandate_income_milli: [i64; 3],

    // Stocks.
    pub finance_milli: i64,
    pub mandate_milli: i64,
    /// One-shot icon grants from claimed opportunities.
    pub bonus_icons: [[i64; 4]; 3],

    // Project lifecycle.
    pub queue: Vec<QueuedProject>,
    pub active: Vec<ActiveProject>,
    pub completed: Vec<CompletedProject>,
    /// Completions ever, per project index — the cost-curve input. Never
    /// decremented, including on decommission.
    pub completions_ever: Vec<u32>,
    /// Unlock ratchet, per project index: once granted, an unlock persists
    /// even if its granting project is decommissioned (the knowledge remains).
    pub unlocked: Vec<bool>,

    // Opportunities.
    pub opportunities: Vec<OpenOpportunity>,
    pub next_opportunity_tick: u64,

    // Outcome.
    pub victory_tick: Option<u64>,
    /// Why the queue head is stalled, if it is.
    pub last_block: Option<BlockReason>,

    // Derived every tick from the lists above.
    pub sector_emissions_milli: [[i64; 3]; 3],
    pub icons: [[i64; 4]; 3],
    pub finance_delta_milli: i64,
    pub mandate_delta_milli: i64,
    pub mandate_max_milli: i64,
    pub slots_total: u32,

    /// The explanation trace. Derivable from the command log, so it is not
    /// part of the digested state.
    #[serde(skip)]
    pub trace: Vec<TraceEvent>,
}

impl RunState {
    pub fn total_emissions_milli(&self) -> i64 {
        self.sector_emissions_milli
            .iter()
            .flat_map(|row| row.iter())
            .sum()
    }

    pub fn continent_emissions_milli(&self, continent: Continent) -> i64 {
        self.sector_emissions_milli[continent.index()].iter().sum()
    }

    /// The victory condition: gross global emissions at zero with
    /// non-negative Finance and Mandate deltas.
    pub fn victory_condition_met(&self) -> bool {
        self.total_emissions_milli() == 0
            && self.finance_delta_milli >= 0
            && self.mandate_delta_milli >= 0
    }
}
