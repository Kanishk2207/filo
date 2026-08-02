//! `filo autostart` — install / remove an OS login service.
//!
//! Each platform gets its own implementation behind `#[cfg]`:
//!
//! - **macOS**: launchd plist in `~/Library/LaunchAgents/`
//! - **Linux**: systemd user unit in `~/.config/systemd/user/`
//! - **Windows**: registry run key in `HKCU\...\Run`
//!
//! The service runs `filo start --foreground`, which means the service
//! manager owns the process lifecycle (restart, stop, etc.).

use std::path::PathBuf;

use anyhow::{Context, Result};

// ── macOS (launchd) ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
const LABEL: &str = "com.filo.daemon";

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", LABEL)))
}

#[cfg(target_os = "macos")]
fn plist_contents(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>start</string>
        <string>--foreground</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
        label = LABEL,
        exe = exe,
    )
}

#[cfg(target_os = "macos")]
pub fn enable() -> Result<()> {
    let exe = std::env::current_exe()?;
    let path = plist_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, plist_contents(&exe.display().to_string()))?;

    let status = std::process::Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&path)
        .status()
        .context("running launchctl load")?;

    if status.success() {
        println!("Autostart enabled. filo will start on login.");
        println!("  plist: {}", path.display());
    } else {
        anyhow::bail!("launchctl load failed (exit {})", status);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn disable() -> Result<()> {
    let path = plist_path()?;
    if !path.exists() {
        println!("Autostart is not enabled.");
        return Ok(());
    }

    let _ = std::process::Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&path)
        .status();

    std::fs::remove_file(&path)?;
    println!("Autostart disabled. Plist removed.");
    Ok(())
}

/// Whether the login service is currently installed.
///
/// Lets callers (notably `filo init`) skip a redundant enable, which on
/// macOS would fail outright because `launchctl load` rejects an
/// already-loaded service.
#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    plist_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn status() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        println!("Autostart: enabled");
        println!("  plist: {}", path.display());
    } else {
        println!("Autostart: disabled");
    }
    Ok(())
}

// ── Linux (systemd user unit) ──────────────────────────────────────────

#[cfg(target_os = "linux")]
fn unit_path() -> Result<PathBuf> {
    let config = dirs::config_dir().context("could not determine config directory")?;
    Ok(config.join("systemd").join("user").join("filo.service"))
}

#[cfg(target_os = "linux")]
fn unit_contents(exe: &str) -> String {
    format!(
        r#"[Unit]
Description=filo file organizer daemon

[Service]
Type=simple
ExecStart={exe} start --foreground
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        exe = exe,
    )
}

#[cfg(target_os = "linux")]
pub fn enable() -> Result<()> {
    let exe = std::env::current_exe()?;
    let path = unit_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, unit_contents(&exe.display().to_string()))?;

    let reload = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .context("running systemctl daemon-reload")?;
    if !reload.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let enable = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "filo"])
        .status()
        .context("running systemctl enable")?;
    if !enable.success() {
        anyhow::bail!("systemctl enable --now filo failed");
    }

    println!("Autostart enabled. filo will start on login.");
    println!("  unit: {}", path.display());
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn disable() -> Result<()> {
    let path = unit_path()?;
    if !path.exists() {
        println!("Autostart is not enabled.");
        return Ok(());
    }

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "--now", "filo"])
        .status();

    std::fs::remove_file(&path)?;

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    println!("Autostart disabled. Unit file removed.");
    Ok(())
}

/// Whether the login service is currently installed. See the macOS
/// implementation for why callers need this.
#[cfg(target_os = "linux")]
pub fn is_enabled() -> bool {
    unit_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(target_os = "linux")]
pub fn status() -> Result<()> {
    let path = unit_path()?;
    if path.exists() {
        println!("Autostart: enabled");
        println!("  unit: {}", path.display());

        let out = std::process::Command::new("systemctl")
            .args(["--user", "is-active", "filo"])
            .output();
        if let Ok(o) = out {
            let state = String::from_utf8_lossy(&o.stdout).trim().to_string();
            println!("  service state: {}", state);
        }
    } else {
        println!("Autostart: disabled");
    }
    Ok(())
}

// ── Windows (registry Run key) ─────────────────────────────────────────

#[cfg(target_os = "windows")]
const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(target_os = "windows")]
pub fn enable() -> Result<()> {
    let exe = std::env::current_exe()?;
    let value = format!("\"{}\" start --foreground", exe.display());

    let status = std::process::Command::new("reg")
        .args([
            "add", REG_KEY, "/v", "filo", "/t", "REG_SZ", "/d", &value, "/f",
        ])
        .status()
        .context("running reg add")?;

    if status.success() {
        println!("Autostart enabled. filo will start on login.");
        println!("  registry: {}\\filo", REG_KEY);
    } else {
        anyhow::bail!("reg add failed (exit {})", status);
    }
    Ok(())
}

/// Whether the login service is currently installed. See the macOS
/// implementation for why callers need this.
#[cfg(target_os = "windows")]
pub fn is_enabled() -> bool {
    std::process::Command::new("reg")
        .args(["query", REG_KEY, "/v", "filo"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
pub fn disable() -> Result<()> {
    let status = std::process::Command::new("reg")
        .args(["delete", REG_KEY, "/v", "filo", "/f"])
        .status()
        .context("running reg delete")?;

    if status.success() {
        println!("Autostart disabled.");
    } else {
        println!("Autostart was not enabled (or registry key missing).");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn status() -> Result<()> {
    let output = std::process::Command::new("reg")
        .args(["query", REG_KEY, "/v", "filo"])
        .output()
        .context("running reg query")?;

    if output.status.success() {
        println!("Autostart: enabled");
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("filo") {
                println!("  {}", line.trim());
            }
        }
    } else {
        println!("Autostart: disabled");
    }
    Ok(())
}
