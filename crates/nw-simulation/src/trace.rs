//! The explanation contract's data layer: structured trace events for every
//! visible state change, and the block reasons the stalled queue head reports.
//! Previews, notifications, the causal recap, and debug inspection all read
//! this one representation. Events are structured (no free prose), so clients
//! own wording and translation.

use nw_content::world::{Continent, Icon};
use serde::{Deserialize, Serialize};

/// Why the FIFO queue head has not started. The queue stalls and explains why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockReason {
    NoFreeSlot,
    NotUnlocked,
    MissingIcon { icon: Icon, needed: i64, have: i64 },
    MissingProject { project: String },
    RepeatLimitReached,
    InsufficientFinance { needed_milli: i64, have_milli: i64 },
    InsufficientMandate { needed_milli: i64, have_milli: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub tick: u64,
    pub kind: TraceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceKind {
    ProjectStarted {
        project: String,
        lead: Continent,
    },
    /// `bonus_permille` is the scaling+breakpoint bonus locked in at
    /// completion; the full modifier list is reproducible via the calc layer.
    ProjectCompleted {
        project: String,
        lead: Continent,
        bonus_permille: i64,
    },
    ProjectCancelled {
        project: String,
        lead: Continent,
    },
    ProjectDecommissioned {
        project: String,
        lead: Continent,
    },
    SlotUnlocked {
        total: u32,
    },
    OpportunityOpened {
        id: String,
        continent: Continent,
    },
    OpportunityClaimed {
        id: String,
    },
    OpportunityExpired {
        id: String,
    },
    QueueBlocked {
        reason: BlockReason,
    },
    VictoryReached,
}
