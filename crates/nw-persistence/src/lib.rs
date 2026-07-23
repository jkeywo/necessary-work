//! Persistence: snapshots, command logs, completion records, and the archive.
//!
//! The authoritative validation record is a scenario identifier, seed, and the
//! ordered log of accepted and rejected commands with their ticks. Validation
//! recreates the initial state, replays the commands, checks periodic state
//! hashes, and confirms the final hash and victory tick. A snapshot may cache
//! state for fast loading, but the command log is authoritative.
//!
//! Compatibility is version-specific: old records are archived, not converted.
//!
//! Scaffold status: the record format and replay-validation harness are grown
//! against `pasm/spec/core/simulation-contract.yaml`.

use nw_simulation::Tick;

/// The outcome of replaying a command log against a fresh initial state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationOutcome {
    /// Every periodic hash and the final hash matched; victory reached at this tick.
    Valid { completion_tick: Tick },
    /// A state hash diverged from the recorded value at this tick.
    HashMismatch { at: Tick },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_carries_completion_tick() {
        let outcome = ValidationOutcome::Valid {
            completion_tick: Tick(42),
        };
        assert_eq!(
            outcome,
            ValidationOutcome::Valid {
                completion_tick: Tick(42)
            }
        );
    }
}
