//! Daemon lifecycle management.
//!
//! Owns the PID file, the reload sentinel, process spawning, and
//! process termination. All platform-specific code for daemon management
//! is consolidated here behind `#[cfg]` blocks.

// This module requires `unsafe` for two POSIX calls: `libc::setsid`
// (daemon detach) and `libc::kill` (process liveness / termination).
// Both are async-signal-safe and have no complex preconditions.
#![allow(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::errors::DaemonError;

const PID_FILENAME: &str = "filo.pid";
const RELOAD_SENTINEL: &str = "filo.reload";

fn data_dir() -> Result<PathBuf, DaemonError> {
    let base = dirs::data_dir().ok_or(DaemonError::NoDataDir)?;
    let dir = base.join("filo");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn pid_path() -> Result<PathBuf, DaemonError> {
    Ok(data_dir()?.join(PID_FILENAME))
}

pub fn reload_sentinel_path() -> Result<PathBuf, DaemonError> {
    Ok(data_dir()?.join(RELOAD_SENTINEL))
}

pub fn write_pid_file() -> Result<(), DaemonError> {
    let path = pid_path()?;
    let pid = std::process::id();
    fs::write(&path, pid.to_string())?;
    log::debug!("wrote PID {} to {}", pid, path.display());
    Ok(())
}

pub fn remove_pid_file() -> Result<(), DaemonError> {
    let path = pid_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn read_pid() -> Result<Option<u32>, DaemonError> {
    let path = pid_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let pid: u32 = raw
        .trim()
        .parse()
        .map_err(|_| DaemonError::PidFileCorrupt { path: path.clone() })?;
    Ok(Some(pid))
}

/// Returns `true` if a filo daemon is running (PID file exists and the
/// process is alive).
pub fn is_running() -> bool {
    match read_pid() {
        Ok(Some(pid)) => is_process_alive(pid),
        _ => false,
    }
}

/// Returns the PID of the running daemon, or `None`.
pub fn running_pid() -> Option<u32> {
    match read_pid() {
        Ok(Some(pid)) if is_process_alive(pid) => Some(pid),
        _ => None,
    }
}

/// Spawn a fully detached daemon process that runs `filo start --foreground`.
///
/// The parent returns immediately. The child writes its own PID file on
/// startup (inside `watcher::run`).
pub fn spawn_daemon() -> Result<(), DaemonError> {
    if let Some(pid) = running_pid() {
        return Err(DaemonError::AlreadyRunning(pid));
    }

    let exe = std::env::current_exe().map_err(|source| DaemonError::SpawnFailed { source })?;

    let mut cmd = Command::new(exe);
    cmd.args(["start", "--foreground", "--no-scan"]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and has no preconditions
        // beyond being called in the child before exec. It detaches the
        // daemon from the controlling terminal so it survives the parent
        // shell exiting.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()
        .map_err(|source| DaemonError::SpawnFailed { source })?;

    // Give the daemon a moment to write its PID file so the caller can
    // immediately query `is_running()`.
    thread::sleep(Duration::from_millis(300));

    Ok(())
}

/// Kill the running daemon. Reads the PID, sends SIGTERM (Unix) or
/// taskkill (Windows), waits briefly, then cleans up the PID file.
pub fn kill_daemon() -> Result<(), DaemonError> {
    let pid = match running_pid() {
        Some(pid) => pid,
        None => return Err(DaemonError::NotRunning),
    };

    kill_process(pid)?;

    // Wait for the process to actually exit.
    for _ in 0..20 {
        if !is_process_alive(pid) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    remove_pid_file()?;

    // Also clean up the reload sentinel if it exists.
    if let Ok(sentinel) = reload_sentinel_path() {
        let _ = fs::remove_file(sentinel);
    }

    Ok(())
}

/// Create the reload sentinel file so the daemon picks up the signal on
/// its next poll cycle.
pub fn signal_reload() -> Result<(), DaemonError> {
    if !is_running() {
        return Err(DaemonError::NotRunning);
    }
    let path = reload_sentinel_path()?;
    fs::write(&path, "reload")?;
    Ok(())
}

// ── Platform-specific helpers ──────────────────────────────────────────

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            out.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

#[cfg(unix)]
fn kill_process(pid: u32) -> Result<(), DaemonError> {
    let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if ret != 0 {
        return Err(DaemonError::KillFailed {
            pid,
            source: std::io::Error::last_os_error(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn kill_process(pid: u32) -> Result<(), DaemonError> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| DaemonError::KillFailed { pid, source })?;
    if !status.success() {
        return Err(DaemonError::KillFailed {
            pid,
            source: std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("taskkill exited with {}", status),
            ),
        });
    }
    Ok(())
}
