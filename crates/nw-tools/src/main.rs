//! `nw-tools` — content linting and balance reports over the authored
//! catalogue. It reads content only (never the client or the live simulation
//! state) so it can run in CI as a gate on authored data.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nw-tools", about = "Content linting and balance reports")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the authored content catalogue's schema and cross-references.
    Lint,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint => {
            let catalogue = nw_content::Catalogue::load().map_err(anyhow::Error::msg)?;
            let issues = catalogue.validate();
            if issues.is_empty() {
                println!(
                    "content ok: {} projects, {} opportunities, version {:016x}",
                    catalogue.projects.len(),
                    catalogue.opportunities.len(),
                    catalogue.content_version
                );
            } else {
                for issue in &issues {
                    println!("content issue: {issue:?}");
                }
                anyhow::bail!("{} content issue(s) found", issues.len());
            }
        }
    }
    Ok(())
}
