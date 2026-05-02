//! filo — a safe, predictable, reversible folder organizer.
//!
//! This is the library crate. The `filo` binary (see `src/main.rs`) is a
//! thin CLI over this API, so everything you can do from the command line
//! can also be driven programmatically — useful for integration tests,
//! GUI frontends, or embedding the engine in another tool.
//!
//! # Module layout
//!
//! - [`cli`]        — clap argument definitions (no logic).
//! - [`commands`]   — one module per `filo <verb>`; orchestrates the engine.
//! - [`config`]     — on-disk config schema, load / save.
//! - [`errors`]     — typed errors per domain.
//! - [`logging`]    — env_logger + file output setup.
//! - [`organizer`]  — the engine: rules, renaming, duplicate detection, safe move.
//! - [`watcher`]    — debounced notify-based folder watcher.

#![forbid(unsafe_code)]

pub mod cli;
pub mod commands;
pub mod config;
pub mod errors;
pub mod logging;
pub mod organizer;
pub mod watcher;
