//! Cross-platform folder watcher with hot-reload support.
//!
//! `notify` delivers raw filesystem events, which arrive in bursts for
//! operations like "download finished and renamed" or "editor saved a
//! file atomically via temp+rename". We coalesce these with a pending-map
//! debounce: every event updates the file's last-seen timestamp, and a
//! file is only processed once it's been idle for `debounce_ms`.
//!
//! ## Hot reload
//!
//! On every poll cycle the event loop checks for a reload sentinel file
//! written by `filo refresh`. When found, the config is re-read from disk,
//! existing watches are torn down, and new watches are established — all
//! without restarting the process. There is zero gap in monitoring.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::Config;
use crate::daemon;
use crate::errors::WatcherError;
use crate::organizer;

fn poll_interval(debounce: Duration) -> Duration {
    debounce.min(Duration::from_millis(250))
}

/// Main entry point for the foreground watcher loop.
///
/// Takes ownership of the initial config because it may be replaced by a
/// hot reload during the lifetime of the process. Writes its PID file on
/// entry and removes it on exit.
pub fn run(initial_config: Config) -> Result<(), WatcherError> {
    if initial_config.watch.folders.is_empty() {
        log::warn!("no folders configured for watching; run `filo init` to add some");
        return Ok(());
    }

    daemon::write_pid_file()?;

    // Ensure we clean up the PID file if we exit for any reason.
    let result = run_inner(initial_config);

    let _ = daemon::remove_pid_file();
    result
}

fn run_inner(initial_config: Config) -> Result<(), WatcherError> {
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;

    let mut config = initial_config;
    let mut watched: Vec<PathBuf> = Vec::new();

    setup_watches(&mut watcher, &config, &mut watched);

    let reload_path = daemon::reload_sentinel_path().map_err(WatcherError::Daemon)?;
    let debounce = Duration::from_millis(config.watch.debounce_ms);
    let poll = poll_interval(debounce);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    log::info!("filo watcher started (PID {})", std::process::id());

    loop {
        // ── Hot-reload check ───────────────────────────────────────
        if reload_path.exists() {
            fs::remove_file(&reload_path).ok();
            match Config::load() {
                Ok(new_cfg) => {
                    teardown_watches(&mut watcher, &watched);
                    watched.clear();
                    pending.clear();
                    config = new_cfg;
                    setup_watches(&mut watcher, &config, &mut watched);
                    log::info!("config reloaded successfully");
                }
                Err(e) => log::error!("config reload failed, keeping current config: {}", e),
            }
        }

        // ── Event ingestion ────────────────────────────────────────
        match rx.recv_timeout(poll) {
            Ok(Ok(event)) => record(event, &mut pending),
            Ok(Err(e)) => log::error!("watcher error: {}", e),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(WatcherError::Disconnected),
        }

        drain_ready(
            &mut pending,
            Duration::from_millis(config.watch.debounce_ms),
            &config,
        );
    }
}

fn setup_watches(watcher: &mut RecommendedWatcher, config: &Config, watched: &mut Vec<PathBuf>) {
    for folder in &config.watch.folders {
        if let Err(e) = watcher.watch(folder, RecursiveMode::NonRecursive) {
            log::error!("failed to watch {}: {}", folder.display(), e);
            continue;
        }
        log::info!("watching {}", folder.display());
        watched.push(folder.clone());
    }
}

fn teardown_watches(watcher: &mut RecommendedWatcher, watched: &[PathBuf]) {
    for folder in watched {
        if let Err(e) = watcher.unwatch(folder) {
            log::debug!(
                "unwatch {} (may already be removed): {}",
                folder.display(),
                e
            );
        }
    }
}

fn record(event: Event, pending: &mut HashMap<PathBuf, Instant>) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            let now = Instant::now();
            for path in event.paths {
                if organizer::should_skip(&path) {
                    continue;
                }
                // Do not gate on `is_file()` here: for some backends a create
                // event can arrive before the filesystem reflects the new file.
                // `process()` re-checks `is_file()` after debounce.
                pending.insert(path, now);
            }
        }
        _ => {}
    }
}

fn drain_ready(pending: &mut HashMap<PathBuf, Instant>, debounce: Duration, config: &Config) {
    let now = Instant::now();
    let ready: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, t)| now.duration_since(**t) >= debounce)
        .map(|(p, _)| p.clone())
        .collect();

    for path in ready {
        pending.remove(&path);
        if let Err(e) = process(&path, config) {
            log::error!("processing {} failed: {}", path.display(), e);
        }
    }
}

fn process(path: &Path, config: &Config) -> anyhow::Result<()> {
    if !path.is_file() {
        return Ok(());
    }

    let root = config
        .watch
        .folders
        .iter()
        .find(|f| path.starts_with(f))
        .cloned();
    let Some(root) = root else {
        log::debug!("ignoring event outside any watch root: {}", path.display());
        return Ok(());
    };

    if !organizer::wait_for_stable(path, 3, Duration::from_millis(250))? {
        log::warn!(
            "file {} is still changing; will retry on next event",
            path.display()
        );
        return Ok(());
    }

    let plan = organizer::plan(path, &root, config)?;
    announce(&plan);
    organizer::execute(&plan)?;
    Ok(())
}

fn announce(plan: &organizer::Plan) {
    match &plan.action {
        organizer::Action::Move { destination } => {
            log::info!(
                "move: {} -> {}",
                plan.source.display(),
                destination.display()
            );
        }
        organizer::Action::SkipDuplicate { of } => {
            log::info!(
                "skip duplicate: {} is identical to {}",
                plan.source.display(),
                of.display()
            );
        }
        organizer::Action::MoveDuplicate { destination, of } => {
            log::info!(
                "duplicate: {} -> {} (identical to {})",
                plan.source.display(),
                destination.display(),
                of.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::CreateKind;

    #[test]
    fn record_enqueues_create_even_if_path_not_present_yet() {
        let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
        let path =
            std::env::temp_dir().join(format!("filo-watcher-record-test-{}", std::process::id()));
        let event = Event::new(EventKind::Create(CreateKind::File)).add_path(path.clone());

        record(event, &mut pending);

        assert!(pending.contains_key(&path));
    }
}
