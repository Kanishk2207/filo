//! `filo watch` — add, remove, or list watched folders.
//!
//! Modifications are written to `config.toml` immediately. If the daemon
//! is running, a reload signal is sent so the changes take effect without
//! restarting.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::daemon;

pub fn add(paths: Vec<PathBuf>) -> Result<()> {
    let mut config = Config::load().context("loading config")?;
    let mut added = 0usize;

    for raw in paths {
        let path = match raw.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                println!(
                    "  skip: {} (does not exist or is not accessible)",
                    raw.display()
                );
                continue;
            }
        };
        if !path.is_dir() {
            println!("  skip: {} (not a directory)", path.display());
            continue;
        }
        if config.add_folder(path.clone()) {
            println!("  added: {}", path.display());
            added += 1;
        } else {
            println!("  already watched: {}", path.display());
        }
    }

    if added > 0 {
        config.save().context("saving config")?;
        signal_if_running();
    }

    Ok(())
}

pub fn remove(paths: Vec<PathBuf>) -> Result<()> {
    let mut config = Config::load().context("loading config")?;
    let mut removed = 0usize;

    for raw in paths {
        // Try canonical first, then fall back to the raw path so users
        // can remove entries that no longer exist on disk.
        let path = raw.canonicalize().unwrap_or_else(|_| raw.clone());
        if config.remove_folder(&path) {
            println!("  removed: {}", path.display());
            removed += 1;
        } else {
            println!("  not in watch list: {}", path.display());
        }
    }

    if removed > 0 {
        config.save().context("saving config")?;
        signal_if_running();
    }

    Ok(())
}

pub fn list() -> Result<()> {
    let config = Config::load().context("loading config")?;

    if config.watch.folders.is_empty() {
        println!("No folders configured. Use `filo watch add <path>` or `filo init`.");
        return Ok(());
    }

    println!("Watched folders:");
    for folder in &config.watch.folders {
        let status = if folder.is_dir() { "" } else { "  (missing)" };
        println!("  {}{}", folder.display(), status);
    }

    Ok(())
}

fn signal_if_running() {
    if daemon::is_running() {
        match daemon::signal_reload() {
            Ok(()) => println!("Daemon notified — changes will take effect momentarily."),
            Err(e) => log::warn!("failed to signal daemon reload: {}", e),
        }
    }
}
