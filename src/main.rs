//! filo binary entry point. Parses CLI, initializes logging, dispatches
//! to the appropriate command handler in the `filo` library crate.

use anyhow::Result;
use clap::Parser;

use filo::cli::{AutostartAction, Cli, Command, WatchAction};
use filo::commands::{arrange, autostart, init, preview, refresh, scan, start, stop, watch_cmd};
use filo::config::Config;
use filo::logging;

fn main() -> Result<()> {
    let log_path = logging::init()?;
    log::debug!("filo started; log file: {}", log_path.display());

    let cli = Cli::parse();
    match cli.command {
        Command::Init => init::run()?,

        Command::Start {
            rename,
            foreground,
            no_scan,
        } => {
            let config = Config::load()?;
            start::run(&config, rename, foreground, no_scan)?;
        }

        Command::Stop => stop::run()?,

        Command::Refresh => refresh::run()?,

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

        Command::Watch { action } => match action {
            WatchAction::Add { paths } => watch_cmd::add(paths)?,
            WatchAction::Remove { paths } => watch_cmd::remove(paths)?,
            WatchAction::List => watch_cmd::list()?,
        },

        Command::Autostart { action } => match action {
            AutostartAction::Enable => autostart::enable()?,
            AutostartAction::Disable => autostart::disable()?,
            AutostartAction::Status => autostart::status()?,
        },
    }

    Ok(())
}
