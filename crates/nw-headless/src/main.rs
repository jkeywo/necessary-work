//! `nw` — the headless developer toolchain: deterministic replay validation,
//! seeded bots, batch balance runs, and benchmark timing. It advances the
//! simulation without wall-clock waiting, so it never needs a window or a clock.

use clap::{Parser, Subcommand};

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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::World => {
            use nw_simulation::{Continent, Icon, Sector};
            println!("Continents: {:?}", Continent::ALL);
            println!("Sectors:    {:?}", Sector::ALL);
            println!("Icons:      {:?}", Icon::ALL);
        }
    }
    Ok(())
}
