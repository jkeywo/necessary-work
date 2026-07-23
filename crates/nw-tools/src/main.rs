//! `nw-tools` — content linting and balance reports over the authored catalogue.
//! It reads content only (never the client or the live simulation state) so it
//! can run in CI as a gate on authored data.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nw-tools", about = "Content linting and balance reports")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the authored content catalogue's cross-references and scopes.
    Lint,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint => {
            let issues = nw_content::Catalogue::default().validate();
            if issues.is_empty() {
                println!("content: no issues");
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
