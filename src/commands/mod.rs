//! Subcommand handlers — one module per `filo <verb>`.
//!
//! Each handler is the top-level orchestration for its command: it loads
//! state, runs the engine, and prints user-facing output. No business
//! logic lives here — that belongs in `organizer`, `watcher`, `config`.

pub mod arrange;
pub mod init;
pub mod preview;
pub mod scan;
pub mod start;
