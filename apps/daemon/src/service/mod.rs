//! Service management — install, uninstall, and status for the `clawd` daemon.
//!
//! Platform behaviour:
//! - **macOS**: LaunchAgent plist at `~/Library/LaunchAgents/com.clawde.clawd.plist`
//! - **Linux**: systemd user unit at `~/.config/systemd/user/clawd.service`
//! - **Windows**: Windows Service via `sc` CLI (requires Administrator)

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Install and start the daemon as a platform service.
pub fn install() -> Result<()> {
    let exe = std::env::current_exe().context("cannot determine clawd executable path")?;

    #[cfg(target_os = "macos")]
    return macos_install(&exe);

    #[cfg(target_os = "linux")]
    return linux_install(&exe);

    #[cfg(target_os = "windows")]
    return windows_install(&exe);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service install not supported on this platform")
    }
}

/// Stop and remove the platform service.
pub fn uninstall() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos_uninstall();

    #[cfg(target_os = "linux")]
    return linux_uninstall();

    #[cfg(target_os = "windows")]
    return windows_uninstall();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service uninstall not supported on this platform")
    }
}

/// Start the daemon via the OS service manager.
pub fn start() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let path = plist_path()?;
        if !path.exists() {
            anyhow::bail!("clawd is not installed — run `clawd service install` first");
        }
        run_cmd("launchctl", &["load", "-w", &path.to_string_lossy()])?;
        println!("clawd started.");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        run_cmd("systemctl", &["--user", "start", "clawd"])?;
        println!("clawd started.");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        run_cmd("sc", &["start", WINDOWS_SERVICE_NAME])?;
        println!("clawd started.");
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service start not supported on this platform")
    }
}

/// Stop the daemon via the OS service manager.
pub fn stop() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let path = plist_path()?;
        if !path.exists() {
            anyhow::bail!("clawd is not installed — run `clawd service install` first");
        }
        run_cmd("launchctl", &["unload", &path.to_string_lossy()])?;
        println!("clawd stopped.");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        run_cmd("systemctl", &["--user", "stop", "clawd"])?;
        println!("clawd stopped.");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        run_cmd("sc", &["stop", WINDOWS_SERVICE_NAME])?;
        println!("clawd stopped.");
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service stop not supported on this platform")
    }
}

/// Restart the daemon via the OS service manager.
pub fn restart() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let path = plist_path()?;
        if !path.exists() {
            anyhow::bail!("clawd is not installed — run `clawd service install` first");
        }
        // launchctl has no restart — unload then load
        let _ = run_cmd("launchctl", &["unload", &path.to_string_lossy()]);
        run_cmd("launchctl", &["load", "-w", &path.to_string_lossy()])?;
        println!("clawd restarted.");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        run_cmd("systemctl", &["--user", "restart", "clawd"])?;
        println!("clawd restarted.");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = run_cmd("sc", &["stop", WINDOWS_SERVICE_NAME]);
        run_cmd("sc", &["start", WINDOWS_SERVICE_NAME])?;
        println!("clawd restarted.");
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service restart not supported on this platform")
    }
}

/// Print daemon service status.
pub fn status() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos_status();

    #[cfg(target_os = "linux")]
    return linux_status();

    #[cfg(target_os = "windows")]
    return windows_status();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("service status not supported on this platform")
    }
}

// ─── macOS (LaunchAgent) ──────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join("com.clawde.clawd.plist"))
}

#[cfg(target_os = "macos")]
fn macos_install(exe: &std::path::Path) -> Result<()> {
    let path = plist_path()?;
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
    <string>{exe}</string>
    <string>serve</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>/tmp/clawd.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/clawd.log</string>
</dict>
</plist>"#,
        exe = exe.display()
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, plist)?;
    info!(plist = %path.display(), "LaunchAgent written");

    run_cmd("launchctl", &["load", "-w", &path.to_string_lossy()])?;
    println!("clawd installed and started (LaunchAgent).");
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_uninstall() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        let _ = run_cmd("launchctl", &["unload", "-w", &path.to_string_lossy()]);
        std::fs::remove_file(&path)?;
        println!("clawd uninstalled.");
    } else {
        println!("clawd is not installed.");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_status() -> Result<()> {
    let out = std::process::Command::new("launchctl")
        .args(["list", "com.clawde.clawd"])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    if out.status.success() && !text.contains("Could not find") {
        println!("clawd is running.\n{text}");
    } else {
        println!("clawd is not running.");
    }
    Ok(())
}

// ─── Linux (systemd user) ──────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn unit_path() -> Result<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.config")
    });
    Ok(PathBuf::from(config_home)
        .join("systemd")
        .join("user")
        .join("clawd.service"))
}

#[cfg(target_os = "linux")]
fn linux_install(exe: &std::path::Path) -> Result<()> {
    let path = unit_path()?;
    let unit = format!(
        r#"[Unit]
Description=ClawDE Host Daemon
After=network.target

[Service]
Type=simple
ExecStart={exe} serve
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        exe = exe.display()
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, unit)?;
    info!(unit = %path.display(), "systemd unit written");

    run_cmd("systemctl", &["--user", "daemon-reload"])?;
    run_cmd("systemctl", &["--user", "enable", "--now", "clawd"])?;
    println!("clawd installed and started (systemd user unit).");
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_uninstall() -> Result<()> {
    let _ = run_cmd("systemctl", &["--user", "disable", "--now", "clawd"]);
    let path = unit_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
        let _ = run_cmd("systemctl", &["--user", "daemon-reload"]);
    }
    println!("clawd uninstalled.");
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_status() -> Result<()> {
    let out = std::process::Command::new("systemctl")
        .args(["--user", "status", "clawd"])
        .output()?;
    println!("{}", String::from_utf8_lossy(&out.stdout));
    Ok(())
}

// ─── Windows (sc.exe / Windows Service) ──────────────────────────────────────

#[cfg(target_os = "windows")]
const WINDOWS_SERVICE_NAME: &str = "clawd";

#[cfg(target_os = "windows")]
fn windows_install(exe: &std::path::Path) -> Result<()> {
    run_cmd(
        "sc",
        &[
            "create",
            WINDOWS_SERVICE_NAME,
            "binPath=",
            &format!("{} serve", exe.display()),
            "start=",
            "auto",
            "DisplayName=",
            "ClawDE Host Daemon",
        ],
    )?;
    run_cmd("sc", &["start", WINDOWS_SERVICE_NAME])?;
    println!("clawd installed and started (Windows Service).");
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_uninstall() -> Result<()> {
    let _ = run_cmd("sc", &["stop", WINDOWS_SERVICE_NAME]);
    run_cmd("sc", &["delete", WINDOWS_SERVICE_NAME])?;
    println!("clawd uninstalled.");
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_status() -> Result<()> {
    let out = std::process::Command::new("sc")
        .args(["query", WINDOWS_SERVICE_NAME])
        .output()?;
    println!("{}", String::from_utf8_lossy(&out.stdout));
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn run_cmd(cmd: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("failed to run `{cmd}`"))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("`{cmd} {}` exited with {}", args.join(" "), status)
    }
}
