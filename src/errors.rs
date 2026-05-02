//! Typed errors for every module boundary.
//!
//! Each domain exposes its own `thiserror` enum so callers can match on the
//! specific failure mode. The binary layer lifts these into `anyhow::Error`
//! for user-facing reporting — see `main.rs`.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine the config directory for this platform")]
    NoConfigDir,

    #[error("config file not found at {0}; run `filo init` to create one")]
    NotFound(PathBuf),

    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write config file {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Error)]
pub enum OrganizerError {
    #[error("source file does not exist: {0}")]
    SourceMissing(PathBuf),

    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to move {from} to {to}: {source}")]
    Move {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    // Defensive: protects the "never overwrite" invariant even if every
    // suffix strategy collides (e.g. during a pathological test).
    #[error("could not derive a unique destination name for {0}")]
    NoUniqueName(PathBuf),
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("watcher channel disconnected unexpectedly")]
    Disconnected,
}
