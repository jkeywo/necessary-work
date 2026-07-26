//! Persistence: run records, state digests, and replay validation.
//!
//! A run's authoritative record is its versions, scenario and seed, the
//! ordered command log (accepted *and* rejected), and periodic state hashes.
//! Validation recreates the initial state, replays the commands, checks every
//! periodic hash, and confirms the final hash. Compatibility is
//! version-specific: mismatched records are rejected outright, never
//! converted.
//!
//! Digests are FNV-1a 64 over the postcard serialization of the run state
//! (minus the derivable explanation trace) — target-independent varint
//! encoding, so native and WASM builds must agree.
//!
//! # This is the fleet's `Run`, not a private format
//!
//! The record is [`vellum_save::Run`] and the replay is
//! [`vellum_replay::Simulation`]. An earlier decision recorded that the
//! replay trait was "deliberately not adopted" because this game's log is
//! tick-stamped real time rather than a turn-by-turn command sequence. That
//! judgement was wrong, and this crate is where it is corrected: the trait
//! fits once the tick is understood as part of the *command* rather than
//! something a driver schedules. `apply` advances to the command's tick and
//! then executes it. The hand-written replay loop that used to live here —
//! command cursor, hash cursor, step budget, stall detection — is gone.

use nw_content::Catalogue;
use nw_simulation::{Command, LoggedCommand, Rejection, RunState, Sim, RULESET_VERSION};
use vellum_replay::Simulation;
use vellum_save::{Ledger, Sample, Sampling, Versions};

pub use vellum_save::{Moved, Verdict};

/// The record format itself. Bumped by hand when the *shape* of a record
/// changes; the rules and content dimensions move on their own.
pub const RECORD_FORMAT: u32 = 1;

/// FNV-1a 64 over the postcard bytes of the authoritative state.
pub fn digest(state: &RunState) -> u64 {
    vellum_digest::digest_postcard(state)
}

/// A run of this game.
pub type RunRecord = vellum_save::Run<LoggedCommand>;

/// What this build was written against.
pub fn versions(catalogue: &Catalogue) -> Versions {
    Versions::new(RECORD_FORMAT, RULESET_VERSION, catalogue.content_version)
}

/// Why a replay did not go the way the record says it went.
///
/// Every variant is a disagreement between two builds about the same log, not
/// a player error — the command was recorded with its outcome, so both an
/// unexpected rejection *and* an unexpected acceptance are divergences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Divergence {
    /// The record says this command was accepted; this build refused it.
    UnexpectedRejection(Rejection),
    /// The record says this command was refused; this build accepted it.
    UnexpectedAcceptance,
    /// The simulation could not reach the tick this command was recorded at.
    /// In practice: it is paused, and the record expects time to have passed.
    Stalled { stuck_at: u64, wanted: u64 },
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Divergence::UnexpectedRejection(rejection) => {
                write!(
                    f,
                    "refused on replay, but the record accepted it: {rejection:?}"
                )
            }
            Divergence::UnexpectedAcceptance => {
                f.write_str("accepted on replay, but the record refused it")
            }
            Divergence::Stalled { stuck_at, wanted } => write!(
                f,
                "stuck at tick {stuck_at}; the record expects a command at {wanted}"
            ),
        }
    }
}

/// A live session: a [`Sim`] that keeps its own divergence ledger.
///
/// The sampling lives here rather than in whatever is driving the run,
/// because the simulation is the only thing that knows what a tick is — and
/// because a replay and a recording must sample at exactly the same moments
/// or the ledger compares nothing.
pub struct Runner {
    pub sim: Sim,
    ledger: Ledger,
    /// The last tick sampled, so a paused tick — which does not advance the
    /// counter — cannot sample the same tick twice, and tick zero is not
    /// sampled at all. Matching the behaviour the records were written with.
    last_sampled: u64,
}

impl Runner {
    pub fn new(catalogue: Catalogue, seed: u64) -> Runner {
        let every = catalogue.scenario.hash_every_ticks.max(1);
        Runner {
            sim: Sim::new(catalogue, seed),
            ledger: Ledger {
                every,
                ..Ledger::default()
            },
            last_sampled: 0,
        }
    }

    /// Advance one tick and sample on cadence.
    pub fn tick(&mut self) {
        self.sim.tick();
        let tick = self.sim.state.tick;
        let digest = digest(&self.sim.state);
        if tick.is_multiple_of(self.ledger.every) && tick != self.last_sampled {
            self.ledger.samples.push(Sample { tick, digest });
            self.last_sampled = tick;
        }
        self.ledger.final_tick = tick;
        self.ledger.final_digest = digest;
    }

    pub fn into_record(self) -> RunRecord {
        RunRecord {
            versions: versions(self.sim.catalogue()),
            scenario: self.sim.catalogue().scenario.id.clone(),
            seed: self.sim.state.seed,
            commands: self.sim.log.clone(),
            ledger: self.ledger,
        }
    }
}

impl Simulation for Runner {
    type Command = LoggedCommand;
    type Rejection = Divergence;

    /// Advance to the command's tick, then execute it — which is what makes a
    /// tick-stamped real-time log a command log, and the whole reason the
    /// fleet's replay trait fits this game after all.
    fn apply(&mut self, logged: &LoggedCommand) -> Result<(), Divergence> {
        self.advance_to(logged.tick);
        if self.sim.state.tick != logged.tick {
            return Err(Divergence::Stalled {
                stuck_at: self.sim.state.tick,
                wanted: logged.tick,
            });
        }
        match (self.sim.execute(logged.command.clone()), &logged.rejection) {
            (Ok(()), None) => Ok(()),
            (Err(_), Some(_)) => Ok(()),
            (Err(rejection), None) => Err(Divergence::UnexpectedRejection(rejection)),
            (Ok(()), Some(_)) => Err(Divergence::UnexpectedAcceptance),
        }
    }

    /// Never: this game's runs are ended by their final tick, not by a state
    /// the log can reach early. Victory does not stop the clock.
    fn is_over(&self) -> bool {
        false
    }

    fn digest(&self) -> u64 {
        digest(&self.sim.state)
    }
}

impl Sampling for Runner {
    fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Advance to `tick`, sampling as it goes.
    ///
    /// Stops early if the simulation is paused, because `Sim::tick` is a no-op
    /// while paused and this would otherwise spin forever. The caller notices
    /// it did not arrive and reports [`Divergence::Stalled`] — a record that
    /// expects time to pass while paused cannot be honest.
    fn advance_to(&mut self, tick: u64) {
        while self.sim.state.tick < tick && !self.sim.state.paused {
            self.tick();
        }
    }
}

/// Replay a record from scratch and check it against its own hashes.
///
/// The version gate runs first and refuses without replaying: a run replayed
/// under changed rules produces a divergence report about a run that was never
/// going to reproduce, which reads like a broken simulation instead of a stale
/// record.
pub fn validate(record: &RunRecord, catalogue: Catalogue) -> Verdict<Divergence> {
    let current = versions(&catalogue);
    let mut runner = Runner::new(catalogue, record.seed);
    vellum_save::verify(record, &current, &mut runner)
}

/// Replay a record and hand back the victory tick it reproduced.
///
/// Separate from [`validate`] because the victory tick is not a *check* — it
/// is part of `RunState` and therefore already inside the final digest, so a
/// record that reproduces has the same victory tick by construction. This
/// just saves the caller replaying it twice to read one field.
pub fn replay(record: &RunRecord, catalogue: Catalogue) -> (Verdict<Divergence>, Option<u64>) {
    let current = versions(&catalogue);
    let mut runner = Runner::new(catalogue, record.seed);
    let verdict = vellum_save::verify(record, &current, &mut runner);
    let victory_tick = runner.sim.state.victory_tick;
    (verdict, victory_tick)
}

/// Run a command through the session, logging it. The recording counterpart of
/// [`Simulation::apply`], which is the replaying one.
impl Runner {
    pub fn execute(&mut self, command: Command) -> Result<(), Rejection> {
        self.sim.execute(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(seed: u64, ticks: u64) -> RunRecord {
        let mut runner = Runner::new(Catalogue::embedded(), seed);
        for _ in 0..ticks {
            runner.tick();
        }
        runner.into_record()
    }

    #[test]
    fn record_round_trips_through_ron() {
        let record = run(3, 600);
        let text = record.to_ron().expect("serializes");
        assert_eq!(RunRecord::from_ron(&text).expect("parses"), record);
    }

    #[test]
    fn an_untouched_run_validates() {
        let record = run(5, 1200);
        assert_eq!(
            validate(&record, Catalogue::embedded()),
            Verdict::Reproduced
        );
    }

    #[test]
    fn version_mismatch_is_rejected_not_converted() {
        let mut record = run(5, 64);
        record.versions.rules = "proto-0".into();
        assert!(matches!(
            validate(&record, Catalogue::embedded()),
            Verdict::Refused(Moved::Rules { .. })
        ));

        let mut record = run(5, 64);
        record.versions.content ^= 0xff;
        assert!(matches!(
            validate(&record, Catalogue::embedded()),
            Verdict::Refused(Moved::Content { .. })
        ));
    }

    /// The reason periodic hashes are recorded at all: a divergence names the
    /// tick to look at rather than only the fact that one happened. These runs
    /// are thousands of ticks long.
    #[test]
    fn a_moved_periodic_hash_is_located() {
        let mut record = run(5, 2000);
        assert!(
            record.ledger.samples.len() > 2,
            "this run should have sampled several times"
        );
        let at = record.ledger.samples[1].tick;
        record.ledger.samples[1].digest ^= 0xff;
        assert!(matches!(
            validate(&record, Catalogue::embedded()),
            Verdict::Diverged { at_tick: Some(tick), .. } if tick == at
        ));
    }

    #[test]
    fn a_moved_final_hash_is_caught() {
        let mut record = run(5, 600);
        record.ledger.final_digest ^= 0xff;
        record.ledger.samples.clear();
        assert!(matches!(
            validate(&record, Catalogue::embedded()),
            Verdict::Diverged { at_tick: None, .. }
        ));
    }

    /// The victory tick is inside the final digest, so a reproduced record has
    /// it by construction — this is the test that lets the record stop
    /// carrying it as a separate field.
    #[test]
    fn a_reproduced_run_brings_its_victory_tick_with_it() {
        let record = run(5, 1200);
        let (verdict, victory_tick) = replay(&record, Catalogue::embedded());
        assert_eq!(verdict, Verdict::Reproduced);

        let mut fresh = Runner::new(Catalogue::embedded(), 5);
        for _ in 0..1200 {
            fresh.tick();
        }
        assert_eq!(victory_tick, fresh.sim.state.victory_tick);
    }
}
