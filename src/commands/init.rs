//! `filo init` — interactive first-run setup.
//!
//! Builds up a fresh [`Config`] by asking the user a small set of questions,
//! writes it to the platform config path, and optionally hands off to the
//! watcher.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect, Select};

use crate::commands::{autostart, scan};
use crate::config::{Config, DuplicateAction};
use crate::daemon;

pub fn run() -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("filo interactive setup");
    println!("======================");
    println!();

    if Config::exists() {
        let overwrite = Confirm::with_theme(&theme)
            .with_prompt("A config already exists. Overwrite it?")
            .default(false)
            .interact()?;
        if !overwrite {
            println!("Aborted. No changes made.");
            return Ok(());
        }
    }

    let mut config = Config::default();

    config.watch.folders = choose_folders(&theme)?;
    if config.watch.folders.is_empty() {
        anyhow::bail!("filo needs at least one folder to watch");
    }

    config.rename.enabled = Confirm::with_theme(&theme)
        .with_prompt("Enable smart file renaming? (lowercase-with-hyphens, strip redundant words)")
        .default(false)
        .interact()?;
    if config.rename.enabled {
        config.rename.prepend_date = Confirm::with_theme(&theme)
            .with_prompt("  Also prepend YYYY-MM-DD to renamed files?")
            .default(false)
            .interact()?;
    }

    let dup_labels = &[
        "Skip (leave the duplicate in place, log it)",
        "Move to a 'Duplicates' subfolder",
    ];
    let dup_choice = Select::with_theme(&theme)
        .with_prompt("How should filo handle duplicate files?")
        .items(dup_labels)
        .default(0)
        .interact()?;
    config.duplicates.action = match dup_choice {
        0 => DuplicateAction::Skip,
        _ => DuplicateAction::Move,
    };

    config.watch.auto_start = Confirm::with_theme(&theme)
        .with_prompt("Start watching automatically after setup?")
        .default(true)
        .interact()?;

    // Asked alongside the other questions so the user decides everything up
    // front, but acted on further down: installing the login service starts a
    // watcher that reads the config, so the config has to be on disk first.
    let want_autostart = Confirm::with_theme(&theme)
        .with_prompt("Start filo automatically on system startup/login?")
        .default(false)
        .interact()?;

    let path = config.save().context("saving config")?;
    println!();
    println!("Config saved to: {}", path.display());
    println!("Run `filo preview` to see planned moves, `filo scan` to organize existing files,");
    println!("or `filo start` to begin watching.");
    if !want_autostart {
        println!("To start filo on every login later, run `filo autostart enable`.");
    }

    // The initial scan runs before anything starts watching, so a one-shot
    // scan and a freshly launched watcher never race over the same files.
    if config.watch.auto_start {
        println!();
        let scan_first = Confirm::with_theme(&theme)
            .with_prompt("Run an initial scan before starting watcher?")
            .default(true)
            .interact()?;
        if scan_first {
            scan::run(&config, false).context("running initial scan before watcher start")?;
        }
    }

    // launchd and systemd start the watcher the moment the service is
    // installed, so find out whether that happened before deciding to start
    // one here.
    let mut watcher_pid = None;
    if want_autostart {
        println!();
        if install_autostart() {
            watcher_pid = wait_for_watcher(Duration::from_secs(2));
            if let Some(pid) = watcher_pid {
                println!("  It also started the watcher now (PID {}).", pid);
            }
        }
    }

    if config.watch.auto_start {
        println!();
        match watcher_pid {
            Some(pid) => println!(
                "Watcher is already running (PID {}). Use `filo stop` to stop it.",
                pid
            ),
            None => start_watcher()?,
        }
    }

    Ok(())
}

/// Install the OS login service, reporting rather than propagating failure.
/// Returns whether the service was installed by this call.
///
/// By this point the config is already written and setup has essentially
/// succeeded, so a service manager that refuses to cooperate must not abort
/// the wizard. The user gets the error and the exact command to retry.
fn install_autostart() -> bool {
    if autostart::is_enabled() {
        println!("Autostart is already enabled; leaving it as it is.");
        return false;
    }
    // `autostart::enable` prints its own success line, including where the
    // service was installed.
    match autostart::enable() {
        Ok(()) => true,
        Err(e) => {
            println!("Could not enable autostart: {:#}", e);
            println!("  Setup is otherwise complete. To retry, run: filo autostart enable");
            false
        }
    }
}

/// Wait briefly for a just-installed login service to bring the watcher up.
///
/// The PID file only appears once the watcher process is running, so checking
/// immediately after `launchctl load` or `systemctl enable --now` usually
/// loses the race. Returns `None` on Windows, where the registry Run key
/// takes effect at the next login rather than now.
fn wait_for_watcher(timeout: Duration) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(pid) = daemon::running_pid() {
            return Some(pid);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Start the background watcher, unless something already started one.
///
/// On macOS and Linux, installing the login service starts the watcher
/// immediately, so spawning another here would be a second watcher on the
/// same folders (and `spawn_daemon` would fail outright).
fn start_watcher() -> Result<()> {
    if let Some(pid) = daemon::running_pid() {
        println!(
            "Watcher is already running (PID {}). Use `filo stop` to stop it.",
            pid
        );
        return Ok(());
    }

    daemon::spawn_daemon().context("starting daemon")?;
    match daemon::running_pid() {
        Some(pid) => println!(
            "Watcher started in background (PID {}). Use `filo stop` to stop it.",
            pid
        ),
        None => println!("Watcher started in background. Use `filo stop` to stop it."),
    }
    Ok(())
}

fn choose_folders(theme: &ColorfulTheme) -> Result<Vec<PathBuf>> {
    let suggestions = suggested_folders();
    let mut chosen: Vec<PathBuf> = if suggestions.is_empty() {
        Vec::new()
    } else {
        let labels: Vec<String> = suggestions
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let defaults = vec![true; suggestions.len().min(1)];
        let defaults = defaults
            .into_iter()
            .chain(std::iter::repeat(false))
            .take(suggestions.len())
            .collect::<Vec<_>>();
        let picks = MultiSelect::with_theme(theme)
            .with_prompt("Which folders should filo watch? (space to toggle, enter to confirm)")
            .items(&labels)
            .defaults(&defaults)
            .interact()?;
        picks.into_iter().map(|i| suggestions[i].clone()).collect()
    };

    loop {
        let add_more = Confirm::with_theme(theme)
            .with_prompt("Add a custom folder path?")
            .default(false)
            .interact()?;
        if !add_more {
            break;
        }
        let raw = prompt_line("Absolute path to folder")?;
        let path = PathBuf::from(raw.trim());
        if path.is_dir() {
            chosen.push(path);
        } else {
            println!("  Path does not exist or is not a directory. Skipping.");
        }
    }

    Ok(chosen)
}

/// Read a single line using the terminal's canonical line mode.
///
/// This avoids key-by-key input handling glitches seen in tmux paste flows
/// with some prompt renderers.
fn prompt_line(prompt: &str) -> Result<String> {
    eprint!("? {} › ", prompt);
    io::stderr().flush().context("flushing prompt")?;

    let mut raw = String::new();
    let read = io::stdin()
        .read_line(&mut raw)
        .context("reading prompt input")?;
    if read == 0 {
        anyhow::bail!("stdin closed while reading prompt input");
    }

    let value = raw.trim_end_matches(['\r', '\n']).to_string();
    eprintln!("✔ {} · {}", prompt, value);
    Ok(value)
}

/// Platform-neutral suggestions via `dirs` — never hardcode `/home` or
/// `C:\Users`.
fn suggested_folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = dirs::download_dir() {
        if p.is_dir() {
            out.push(p);
        }
    }
    if let Some(p) = dirs::desktop_dir() {
        if p.is_dir() {
            out.push(p);
        }
    }
    if let Some(p) = dirs::document_dir() {
        if p.is_dir() {
            out.push(p);
        }
    }
    out
}
