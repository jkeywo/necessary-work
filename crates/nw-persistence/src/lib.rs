//! Persistence: validation records, state digests, and replay validation.
//!
//! A run's authoritative record is its ruleset and content versions, scenario
//! identifier and seed, the ordered command log (accepted and rejected), and
//! periodic state hashes. Validation recreates the initial state, replays the
//! commands, checks every periodic hash, and confirms the final hash and
//! victory tick. Snapshots may cache state for fast loading later; the command
//! log is authoritative. Compatibility is version-specific: mismatched records
//! are rejected outright, never converted.
//!
//! Digests are FNV-1a 64 over the postcard serialization of the run state
//! (minus the derivable explanation trace) — target-independent varint
//! encoding, so native and WASM builds must agree.

use nw_content::Catalogue;
use nw_simulation::{LoggedCommand, RunState, Sim, RULESET_VERSION};
use serde::{Deserialize, Serialize};

/// FNV-1a 64 over the postcard bytes of the authoritative state.
///
/// The fleet's `digest_postcard` — the same bytes and the same hash the
/// in-crate version produced, so adopting it moved no digest anywhere.
pub fn digest(state: &RunState) -> u64 {
    vellum_digest::digest_postcard(state)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickHash {
    pub tick: u64,
    pub hash: u64,
}

/// The authoritative validation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub ruleset_version: String,
    pub content_version: u64,
    pub scenario: String,
    pub seed: u64,
    pub commands: Vec<LoggedCommand>,
    pub hash_every: u64,
    pub hashes: Vec<TickHash>,
    pub final_tick: u64,
    pub final_hash: u64,
    pub victory_tick: Option<u64>,
}

impl RunRecord {
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("record must serialize")
    }

    pub fn from_ron(text: &str) -> Result<RunRecord, String> {
        ron::from_str(text).map_err(|e| format!("record: {e}"))
    }
}

/// A live session harness: a [`Sim`] plus periodic hash sampling, producing a
/// [`RunRecord`] at the end.
pub struct Runner {
    pub sim: Sim,
    hash_every: u64,
    hashes: Vec<TickHash>,
    last_hashed: u64,
}

impl Runner {
    pub fn new(catalogue: Catalogue, seed: u64) -> Runner {
        let hash_every = catalogue.scenario.hash_every_ticks.max(1);
        Runner {
            sim: Sim::new(catalogue, seed),
            hash_every,
            hashes: Vec::new(),
            last_hashed: 0,
        }
    }

    /// Advance one tick and sample the periodic hash on cadence.
    pub fn tick(&mut self) {
        self.sim.tick();
        let tick = self.sim.state.tick;
        if tick.is_multiple_of(self.hash_every) && tick != self.last_hashed {
            self.hashes.push(TickHash {
                tick,
                hash: digest(&self.sim.state),
            });
            self.last_hashed = tick;
        }
    }

    pub fn into_record(self) -> RunRecord {
        let state = &self.sim.state;
        RunRecord {
            ruleset_version: RULESET_VERSION.to_string(),
            content_version: self.sim.catalogue().content_version,
            scenario: self.sim.catalogue().scenario.id.clone(),
            seed: state.seed,
            commands: self.sim.log.clone(),
            hash_every: self.hash_every,
            hashes: self.hashes,
            final_tick: state.tick,
            final_hash: digest(state),
            victory_tick: state.victory_tick,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationOutcome {
    /// Everything replayed and every hash matched.
    Valid { victory_tick: Option<u64> },
    /// Ruleset, content, or scenario version differs: rejected, not converted.
    VersionMismatch,
    /// A replayed command's accept/reject outcome differed from the record.
    CommandMismatch { index: usize },
    /// A periodic hash diverged.
    HashMismatch { tick: u64 },
    /// The final state hash or victory tick diverged.
    FinalStateMismatch,
    /// The record cannot make progress (e.g. ends paused mid-log).
    Stalled,
}

/// Replay a record from scratch and check it against its own hashes.
pub fn validate(record: &RunRecord, catalogue: Catalogue) -> ValidationOutcome {
    if record.ruleset_version != RULESET_VERSION
        || record.content_version != catalogue.content_version
        || record.scenario != catalogue.scenario.id
    {
        return ValidationOutcome::VersionMismatch;
    }

    let mut sim = Sim::new(catalogue, record.seed);
    let mut next_command = 0usize;
    let mut next_hash = 0usize;
    let mut last_hashed = 0u64;
    let hash_every = record.hash_every.max(1);
    let budget = record.final_tick + record.commands.len() as u64 + 16;
    let mut steps = 0u64;

    loop {
        while next_command < record.commands.len()
            && record.commands[next_command].tick == sim.state.tick
        {
            let logged = &record.commands[next_command];
            let result = sim.execute(logged.command.clone());
            if result.is_err() != logged.rejection.is_some() {
                return ValidationOutcome::CommandMismatch {
                    index: next_command,
                };
            }
            next_command += 1;
        }

        if sim.state.tick >= record.final_tick && next_command >= record.commands.len() {
            break;
        }
        if sim.state.paused {
            // All commands for the frozen tick are consumed and the record
            // still expects progress: it cannot be honest.
            return ValidationOutcome::Stalled;
        }

        sim.tick();
        steps += 1;
        if steps > budget {
            return ValidationOutcome::Stalled;
        }

        let tick = sim.state.tick;
        if tick.is_multiple_of(hash_every) && tick != last_hashed {
            last_hashed = tick;
            if next_hash < record.hashes.len() && record.hashes[next_hash].tick == tick {
                if record.hashes[next_hash].hash != digest(&sim.state) {
                    return ValidationOutcome::HashMismatch { tick };
                }
                next_hash += 1;
            }
        }
    }

    if digest(&sim.state) != record.final_hash || sim.state.victory_tick != record.victory_tick {
        return ValidationOutcome::FinalStateMismatch;
    }
    ValidationOutcome::Valid {
        victory_tick: record.victory_tick,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips_through_ron() {
        let mut runner = Runner::new(Catalogue::embedded(), 3);
        for _ in 0..600 {
            runner.tick();
        }
        let record = runner.into_record();
        let text = record.to_ron();
        assert_eq!(RunRecord::from_ron(&text).unwrap(), record);
    }

    #[test]
    fn an_untouched_run_validates() {
        let mut runner = Runner::new(Catalogue::embedded(), 5);
        for _ in 0..1200 {
            runner.tick();
        }
        let record = runner.into_record();
        assert_eq!(
            validate(&record, Catalogue::embedded()),
            ValidationOutcome::Valid { victory_tick: None }
        );
    }

    #[test]
    fn version_mismatch_is_rejected_not_converted() {
        let mut runner = Runner::new(Catalogue::embedded(), 5);
        for _ in 0..64 {
            runner.tick();
        }
        let mut record = runner.into_record();
        record.ruleset_version = "proto-0".into();
        assert_eq!(
            validate(&record, Catalogue::embedded()),
            ValidationOutcome::VersionMismatch
        );
    }
}
