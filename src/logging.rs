//! Logger setup — writes to both stderr (via env_logger) and a rolling
//! append-only file in the platform data dir.
//!
//! Log level is controlled by the `RUST_LOG` environment variable (e.g.
//! `RUST_LOG=debug filo start`) and defaults to `info`.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};

pub fn log_path() -> Result<PathBuf> {
    let base = dirs::data_dir().context("could not determine data directory")?;
    Ok(base.join("filo").join("filo.log"))
}

/// Initialize the global logger. Idempotent at the process level —
/// env_logger panics on a second init, so callers should only call once
/// from `main`.
pub fn init() -> Result<PathBuf> {
    let path = log_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating log directory {}", parent.display()))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening log file {}", path.display()))?;
    let file = Arc::new(Mutex::new(file));
    let file_for_fmt = Arc::clone(&file);

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(move |buf, record| {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let line = format!("{} [{}] {}\n", ts, record.level(), record.args());
            // File output — best-effort. We never want a log failure to
            // propagate and kill the command that emitted the log.
            if let Ok(mut f) = file_for_fmt.lock() {
                let _ = f.write_all(line.as_bytes());
            }
            write!(buf, "{}", line)
        })
        .init();

    Ok(path)
}
