//! `filo stop` — stop the background watcher daemon.

use anyhow::{Context, Result};

use crate::daemon;

pub fn run() -> Result<()> {
    match daemon::running_pid() {
        Some(pid) => {
            daemon::kill_daemon().context("failed to stop daemon")?;
            println!("filo daemon stopped (was PID {}).", pid);
        }
        None => {
            println!("filo daemon is not running.");
        }
    }
    Ok(())
}
