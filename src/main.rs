//! filo binary entry point. Parses CLI, initializes logging, dispatches
//! to the appropriate command handler in the `filo` library crate.

use anyhow::Result;
use clap::Parser;

use filo::cli::{Cli, Command};
use filo::commands::{arrange, init, preview, scan, start};
use filo::config::Config;
use filo::logging;

fn main() -> Result<()> {
    let log_path = logging::init()?;
    log::debug!("filo started; log file: {}", log_path.display());

    let cli = Cli::parse();
    match cli.command {
        Command::Init => init::run()?,
        Command::Start { rename } => {
            let config = Config::load()?;
            start::run(&config, rename)?;
        }
        Command::Scan { rename } => {
            let config = Config::load()?;
            scan::run(&config, rename)?;
        }
        Command::Preview => {
            let config = Config::load()?;
            preview::run(&config)?;
        }
        Command::Arrange {
            sources,
            destination,
            keywords,
            extensions,
            yes,
        } => {
            arrange::run(arrange::ArrangeArgs {
                sources,
                destination,
                keywords,
                extensions,
                yes,
            })?;
        }
    }

    Ok(())
}
