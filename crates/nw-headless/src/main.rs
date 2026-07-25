//! `nw` — the headless developer toolchain: deterministic bot runs, replay
//! validation, batch build-order checks, and world inspection. It advances the
//! simulation without wall-clock waiting, so it never needs a window or clock.

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use nw_content::Catalogue;
use nw_headless::bot::{self, Strategy};
use nw_persistence::{validate, Runner, ValidationOutcome};

#[derive(Parser)]
#[command(
    name = "nw",
    about = "Headless tools for The Necessary Work simulation"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the fixed world structure (continents, sectors, icons).
    World,
    /// Play a full bot run and optionally write its validation record.
    Run {
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value = "deploy-first")]
        strategy: String,
        #[arg(long, default_value_t = 20_000)]
        max_ticks: u64,
        /// Write the run's validation record (RON) to this path.
        #[arg(long)]
        record: Option<std::path::PathBuf>,
    },
    /// Replay-validate a previously written record.
    Validate { record: std::path::PathBuf },
    /// Run both strategies over a seed range; fails unless every run wins.
    Batch {
        #[arg(long, default_value_t = 1)]
        seed_start: u64,
        #[arg(long, default_value_t = 4)]
        count: u64,
        #[arg(long, default_value_t = 20_000)]
        max_ticks: u64,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::World => {
            use nw_simulation::{Continent, Icon, Sector};
            println!("Continents: {:?}", Continent::ALL.map(|c| c.name()));
            println!("Sectors:    {:?}", Sector::ALL.map(|s| s.name()));
            println!("Icons:      {:?}", Icon::ALL.map(|i| i.name()));
        }
        Command::Run {
            seed,
            strategy,
            max_ticks,
            record,
        } => {
            let strategy = Strategy::parse(&strategy)
                .with_context(|| format!("unknown strategy '{strategy}'"))?;
            let mut runner = Runner::new(Catalogue::embedded(), seed);
            let outcome = bot::play(&mut runner, strategy, max_ticks);
            print_summary(&runner, strategy, outcome);
            let run_record = runner.into_record();
            match validate(&run_record, Catalogue::embedded()) {
                ValidationOutcome::Valid { .. } => println!("replay:   valid"),
                other => bail!("replay validation failed: {other:?}"),
            }
            if let Some(path) = record {
                std::fs::write(&path, run_record.to_ron())
                    .with_context(|| format!("writing {}", path.display()))?;
                println!("record:   {}", path.display());
            }
            if outcome.victory_tick.is_none() {
                bail!("no victory within {max_ticks} ticks");
            }
        }
        Command::Validate { record } => {
            let text = std::fs::read_to_string(&record)
                .with_context(|| format!("reading {}", record.display()))?;
            let run_record =
                nw_persistence::RunRecord::from_ron(&text).map_err(anyhow::Error::msg)?;
            match validate(&run_record, Catalogue::embedded()) {
                ValidationOutcome::Valid { victory_tick } => {
                    println!("valid; victory tick: {victory_tick:?}");
                }
                other => bail!("invalid: {other:?}"),
            }
        }
        Command::Batch {
            seed_start,
            count,
            max_ticks,
        } => {
            // The fleet's batch driver: one case per seed x strategy, the
            // records this game's vocabulary, the loop and tallies shared.
            let strategies = [Strategy::DeployFirst, Strategy::CapacityFirst];
            let cases = count * strategies.len() as u64;
            let batch =
                vellum_corpus::drive(0..cases, vellum_corpus::Budget::cases(cases), |case| {
                    let seed = seed_start + case / strategies.len() as u64;
                    let strategy = strategies[(case % strategies.len() as u64) as usize];
                    let mut runner = Runner::new(Catalogue::embedded(), seed);
                    let outcome = bot::play(&mut runner, strategy, max_ticks);
                    let verdict = match outcome.victory_tick {
                        Some(tick) => format!("victory at {tick}"),
                        None => format!(
                            "NO VICTORY ({} milli-Gt left)",
                            runner.sim.state.total_emissions_milli()
                        ),
                    };
                    println!("seed {seed:>4}  {:>14}  {verdict}", strategy.name());
                    outcome.victory_tick.is_some()
                });
            let mut outcomes = vellum_corpus::Tally::new();
            for &won in &batch.records {
                outcomes.add(if won { "victory" } else { "no-victory" });
            }
            let failures = outcomes.count(&"no-victory");
            println!(
                "batch:    {} of {} won ({} permille) in {:.1}s",
                outcomes.count(&"victory"),
                outcomes.total(),
                vellum_corpus::permille(outcomes.count(&"victory"), outcomes.total()),
                batch.elapsed_seconds
            );
            if failures > 0 {
                bail!("{failures} run(s) failed to reach victory");
            }
        }
    }
    Ok(())
}

fn print_summary(runner: &Runner, strategy: Strategy, outcome: bot::Outcome) {
    let state = &runner.sim.state;
    let speed = runner
        .sim
        .catalogue()
        .scenario
        .authored_speed_ticks_per_second
        .max(1);
    println!("strategy: {}", strategy.name());
    println!("seed:     {}", state.seed);
    match outcome.victory_tick {
        Some(tick) => {
            let seconds = tick / u64::from(speed);
            println!(
                "victory:  tick {tick} (~{}m{:02}s at authored speed)",
                seconds / 60,
                seconds % 60
            );
        }
        None => println!(
            "no victory by tick {}; {} milli-Gt remaining",
            outcome.final_tick,
            state.total_emissions_milli()
        ),
    }
    println!(
        "deltas:   finance {:+} milli/tick, mandate {:+} milli/tick",
        state.finance_delta_milli, state.mandate_delta_milli
    );
    println!(
        "built:    {} completions, {} commands logged",
        state.completed.len(),
        runner.sim.log.len()
    );
}
