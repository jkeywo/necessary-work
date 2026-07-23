//! The remaining contract obligations: victory validation, deterministic
//! replay of full runs, version-specific replay rejection, and the Stage 1
//! gate that at least two plausible build orders win.

use nw_content::Catalogue;
use nw_headless::bot::{self, Strategy};
use nw_persistence::{validate, Runner, ValidationOutcome};

const MAX_TICKS: u64 = 20_000;

#[test]
fn both_build_orders_win_and_their_records_validate() {
    let mut logs = Vec::new();
    for strategy in [Strategy::DeployFirst, Strategy::CapacityFirst] {
        let mut runner = Runner::new(Catalogue::embedded(), 7);
        let outcome = bot::play(&mut runner, strategy, MAX_TICKS);
        assert!(
            outcome.victory_tick.is_some(),
            "{} must reach victory (left: {} milli-Gt)",
            strategy.name(),
            runner.sim.state.total_emissions_milli()
        );
        // Victory requires non-negative deltas by definition; check anyway.
        assert!(runner.sim.state.finance_delta_milli >= 0);
        assert!(runner.sim.state.mandate_delta_milli >= 0);

        let record = runner.into_record();
        assert_eq!(
            validate(&record, Catalogue::embedded()),
            ValidationOutcome::Valid {
                victory_tick: record.victory_tick
            },
            "{} record must replay-validate",
            strategy.name()
        );
        logs.push(record.commands);
    }
    // Two genuinely different build orders, not one order twice.
    assert_ne!(logs[0], logs[1]);
}

#[test]
fn tampered_records_are_rejected() {
    let mut runner = Runner::new(Catalogue::embedded(), 3);
    bot::play(&mut runner, Strategy::DeployFirst, 2_000);
    let record = runner.into_record();

    let mut wrong_ruleset = record.clone();
    wrong_ruleset.ruleset_version = "proto-0".into();
    assert_eq!(
        validate(&wrong_ruleset, Catalogue::embedded()),
        ValidationOutcome::VersionMismatch
    );

    let mut wrong_content = record.clone();
    wrong_content.content_version ^= 1;
    assert_eq!(
        validate(&wrong_content, Catalogue::embedded()),
        ValidationOutcome::VersionMismatch
    );

    let mut wrong_final = record.clone();
    wrong_final.final_hash ^= 1;
    assert_eq!(
        validate(&wrong_final, Catalogue::embedded()),
        ValidationOutcome::FinalStateMismatch
    );

    let mut wrong_seed = record;
    wrong_seed.seed ^= 1;
    assert_ne!(
        validate(&wrong_seed, Catalogue::embedded()),
        ValidationOutcome::Valid {
            victory_tick: wrong_seed.victory_tick
        }
    );
}
