//! `filo refresh` — signal the running daemon to hot-reload its config.

use anyhow::{Context, Result};

use crate::daemon;

pub fn run() -> Result<()> {
    if !daemon::is_running() {
        println!("filo daemon is not running. Start it with `filo start`.");
        return Ok(());
    }

    daemon::signal_reload().context("failed to signal reload")?;
    println!("Reload signal sent. The daemon will pick up the new config within a few hundred milliseconds.");
    Ok(())
}
