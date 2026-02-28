// SPDX-License-Identifier: MIT
// Sprint II ST.2 — Daemon auto-start on first launch.
//
// Writes the platform-specific service registration file on first run so the
// daemon starts automatically at login/boot without requiring a manual
// `clawd service install`.
//
// Platforms:
//   macOS  — LaunchAgent plist at ~/Library/LaunchAgents/com.clawde.clawd.plist
//   Linux  — systemd user unit at ~/.config/systemd/user/clawd.service
//   Windows — Windows Service via NSSM (if NSSM is on PATH)
//
// The Flutter desktop app calls `clawd autostart --enable` on first launch
// (via the `AutostartCmd` subcommand) and `clawd autostart --disable` from
// Settings > General > "Start at login" toggle.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Enable the daemon auto-start service for the current user.
///
/// Idempotent — safe to call even if already enabled.
pub fn enable() -> Result<()> {
    let exe = std::env::current_exe().context("cannot determine clawd executable path")?;
    platform_enable(&exe)?;
    info!("clawd autostart enabled");
    println!("Autostart enabled. clawd will start at login.");
    Ok(())
}

/// Disable the daemon auto-start service.
///
/// Idempotent — no-op if not currently enabled.
pub fn disable() -> Result<()> {
    platform_disable()?;
    info!("clawd autostart disabled");
    println!("Autostart disabled.");
    Ok(())
}

/// Returns true if the autostart service is currently installed and enabled.
pub fn is_enabled() -> bool {
    platform_is_enabled()
}

// ─── macOS LaunchAgent ────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join("com.clawde.clawd.plist"))
}

#[cfg(target_os = "macos")]
fn platform_enable(exe: &std::path::Path) -> Result<()> {
    let path = plist_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create LaunchAgents dir")?;
    }

    let exe_str = exe.to_string_lossy();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.clawde.clawd</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_str}</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/tmp/clawd.err</string>
    <key>StandardOutPath</key>
    <string>/tmp/clawd.out</string>
</dict>
</plist>
"#
    );

    std::fs::write(&path, plist).context("write launchd plist")?;
    run_cmd("launchctl", &["load", "-w", &path.to_string_lossy()])?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_disable() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        let _ = run_cmd("launchctl", &["unload", &path.to_string_lossy()]);
        std::fs::remove_file(&path).context("remove plist")?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_is_enabled() -> bool {
    plist_path().map(|p| p.exists()).unwrap_or(false)
}

// ─── Linux systemd user unit ──────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn unit_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("systemd")
        .join("user")
        .join("clawd.service"))
}

#[cfg(target_os = "linux")]
fn platform_enable(exe: &std::path::Path) -> Result<()> {
    let path = unit_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create systemd user dir")?;
    }

    let exe_str = exe.to_string_lossy();
    let unit = format!(
        "[Unit]\n\
         Description=ClawDE Host Daemon\n\
         After=network.target\n\
         \n\
         [Service]\n\
         ExecStart={exe_str} serve\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );

    std::fs::write(&path, unit).context("write systemd unit")?;
    let _ = run_cmd("systemctl", &["--user", "daemon-reload"]);
    run_cmd("systemctl", &["--user", "enable", "--now", "clawd"])?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_disable() -> Result<()> {
    let _ = run_cmd("systemctl", &["--user", "disable", "--now", "clawd"]);
    if let Ok(path) = unit_path() {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            let _ = run_cmd("systemctl", &["--user", "daemon-reload"]);
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_is_enabled() -> bool {
    unit_path().map(|p| p.exists()).unwrap_or(false)
}

// ─── Windows NSSM service ─────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
const WINDOWS_SERVICE_NAME: &str = "ClaWD";

#[cfg(target_os = "windows")]
fn platform_enable(exe: &std::path::Path) -> Result<()> {
    let exe_str = exe.to_string_lossy();
    // Install via NSSM if available; fallback to sc.exe
    if run_cmd("nssm", &["install", WINDOWS_SERVICE_NAME, &exe_str]).is_ok() {
        let _ = run_cmd(
            "nssm",
            &["set", WINDOWS_SERVICE_NAME, "AppParameters", "serve"],
        );
        run_cmd("nssm", &["start", WINDOWS_SERVICE_NAME])?;
    } else {
        // sc.exe fallback (requires admin)
        run_cmd(
            "sc",
            &[
                "create",
                WINDOWS_SERVICE_NAME,
                "binPath=",
                &format!("{exe_str} serve"),
                "start=",
                "auto",
            ],
        )?;
        run_cmd("sc", &["start", WINDOWS_SERVICE_NAME])?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn platform_disable() -> Result<()> {
    let _ = run_cmd("sc", &["stop", WINDOWS_SERVICE_NAME]);
    let _ = run_cmd("sc", &["delete", WINDOWS_SERVICE_NAME]);
    Ok(())
}

#[cfg(target_os = "windows")]
fn platform_is_enabled() -> bool {
    // Query SC to check if registered
    std::process::Command::new("sc")
        .args(["query", WINDOWS_SERVICE_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ─── Unsupported platforms ────────────────────────────────────────────────────

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_enable(_exe: &std::path::Path) -> Result<()> {
    anyhow::bail!("autostart not supported on this platform")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_disable() -> Result<()> {
    anyhow::bail!("autostart not supported on this platform")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_is_enabled() -> bool {
    false
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

fn run_cmd(cmd: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("failed to run `{cmd}`"))?;
    if !status.success() {
        anyhow::bail!("`{cmd}` exited with status {status}");
    }
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_enabled_returns_bool() {
        // Simply verify the function runs without panic.
        let _ = is_enabled();
    }
}
