//! The semantic command boundary. Clients drive the simulation through these
//! commands, never through UI events. Every command returns a structured
//! accepted/rejected result; rejected commands consume no resources, alter no
//! state, and advance no time.

use nw_content::world::Continent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    QueueProject { project: String, lead: Continent },
    RemoveQueuedProject { index: u32 },
    ReorderQueue { from: u32, to: u32 },
    CancelActiveProject { index: u32 },
    DecommissionProject { index: u32 },
    SelectProjectLeadContinent { index: u32, lead: Continent },
    Pause,
    Resume,
    ClaimOpportunity { index: u32 },
}

/// Machine-readable rejection reasons; clients render the human-readable text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    UnknownProject { id: String },
    InvalidIndex,
    AlreadyPaused,
    NotPaused,
}

/// One entry in the authoritative command log: what was asked, at which tick,
/// and whether it was accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggedCommand {
    pub tick: u64,
    pub command: Command,
    pub rejection: Option<Rejection>,
}

impl LoggedCommand {
    pub fn accepted(&self) -> bool {
        self.rejection.is_none()
    }
}
