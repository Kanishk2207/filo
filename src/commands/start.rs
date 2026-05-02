//! `filo start` — scan existing files, then launch the background watcher.
//!
//! By default, `filo start` runs a one-shot scan first, then spawns a
//! detached daemon process. The `--foreground` flag keeps the watcher in
//! the current process (used by service managers and for debugging).

use anyhow::{Context, Result};

use crate::commands::scan;
use crate::config::Config;
use crate::daemon;
use crate::watcher;

pub fn run(config: &Config, rename_override: bool, foreground: bool, no_scan: bool) -> Result<()> {
    let mut config = config.clone();
    if rename_override {
        config.rename.enabled = true;
    }

    if !no_scan {
        println!("Running initial scan...");
        scan::run(&config, rename_override)?;
        println!();
    }

    if foreground {
        println!("filo watcher running in foreground. Press Ctrl-C to stop.");
        watcher::run(config).context("watcher failed")?;
    } else {
        if daemon::is_running() {
            let pid = daemon::running_pid().unwrap_or(0);
            println!(
                "filo daemon is already running (PID {}). Use `filo refresh` to reload config.",
                pid
            );
            return Ok(());
        }
        daemon::spawn_daemon().context("failed to start daemon")?;
        match daemon::running_pid() {
            Some(pid) => println!(
                "filo daemon started (PID {}). Use `filo stop` to stop it.",
                pid
            ),
            None => println!("filo daemon started. Use `filo stop` to stop it."),
        }
    }

    Ok(())
}
