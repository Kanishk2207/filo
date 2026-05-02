//! Clap definitions. Kept intentionally free of business logic — the
//! dispatch happens in `main.rs`, handlers live in `commands::`.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "filo",
    version,
    about = "Your Downloads folder cleans itself — safely, predictably, and reversibly."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Interactive first-time setup.
    Init,

    /// Watch configured folders and organize new files in real time.
    Start {
        /// Enable smart renaming for this run (overrides config).
        #[arg(long)]
        rename: bool,

        /// Run the watcher in the foreground instead of daemonizing.
        /// Used by service managers (launchd, systemd) and for debugging.
        #[arg(long)]
        foreground: bool,

        /// Skip the initial scan of existing files.
        #[arg(long)]
        no_scan: bool,
    },

    /// Stop the background watcher daemon.
    Stop,

    /// Reload config without restarting the daemon.
    Refresh,

    /// Organize files already present in the configured watch folders.
    Scan {
        /// Enable smart renaming for this run (overrides config).
        #[arg(long)]
        rename: bool,
    },

    /// Show what `filo scan` would do, without moving anything.
    Preview,

    /// Manually move files matching keywords/extensions into a destination.
    Arrange {
        /// Source folder(s) to search. Repeat for multiple.
        #[arg(short, long = "source", required = true)]
        sources: Vec<PathBuf>,

        /// Destination folder.
        #[arg(short, long)]
        destination: PathBuf,

        /// Filename keyword (case-insensitive substring). Repeat for multiple.
        #[arg(short, long = "keyword")]
        keywords: Vec<String>,

        /// File extension (no leading dot). Repeat for multiple.
        #[arg(short, long = "extension")]
        extensions: Vec<String>,

        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Manage watched folders.
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },

    /// Manage auto-start on OS login.
    Autostart {
        #[command(subcommand)]
        action: AutostartAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum WatchAction {
    /// Add folder(s) to the watch list.
    Add {
        /// Paths to watch. Must be existing directories.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },

    /// Remove folder(s) from the watch list.
    Remove {
        /// Paths to stop watching.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },

    /// Show currently watched folders.
    List,
}

#[derive(Debug, Subcommand)]
pub enum AutostartAction {
    /// Install filo as a login service so it starts automatically.
    Enable,

    /// Remove the login service.
    Disable,

    /// Show whether autostart is currently enabled.
    Status,
}
