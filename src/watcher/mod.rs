//! Cross-platform folder watcher.
//!
//! `notify` delivers raw filesystem events, which arrive in bursts for
//! operations like "download finished and renamed" or "editor saved a
//! file atomically via temp+rename". We coalesce these with a pending-map
//! debounce: every event updates the file's last-seen timestamp, and a
//! file is only processed once it's been idle for `debounce_ms`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::Config;
use crate::errors::WatcherError;
use crate::organizer;

/// How often the main loop wakes up to scan the pending map, capped to
/// a fraction of the configured debounce so debounced items are processed
/// promptly.
fn poll_interval(debounce: Duration) -> Duration {
    debounce.min(Duration::from_millis(250))
}

pub fn run(config: &Config) -> Result<(), WatcherError> {
    if config.watch.folders.is_empty() {
        log::warn!("no folders configured for watching; run `filo init` to add some");
        return Ok(());
    }

    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        // The send can only fail if the receiver dropped — in which case
        // we're shutting down and dropping the event is fine.
        let _ = tx.send(res);
    })?;

    for folder in &config.watch.folders {
        if let Err(e) = watcher.watch(folder, RecursiveMode::NonRecursive) {
            log::error!("failed to watch {}: {}", folder.display(), e);
            continue;
        }
        log::info!("watching {}", folder.display());
    }

    let debounce = Duration::from_millis(config.watch.debounce_ms);
    let poll = poll_interval(debounce);
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    println!("filo is running. Press Ctrl-C to stop.");

    loop {
        match rx.recv_timeout(poll) {
            Ok(Ok(event)) => record(event, &mut pending),
            Ok(Err(e)) => log::error!("watcher error: {}", e),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(WatcherError::Disconnected),
        }

        drain_ready(&mut pending, debounce, config);
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
                // Only track files — directories are never organized.
                if path.is_file() {
                    pending.insert(path, now);
                }
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

    // Resolve which configured watch folder contains this path — we need
    // it as the `root` the organizer writes category folders under.
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

    // Size-stability: a belt-and-braces check on top of event debouncing.
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
