//! Stable machine identity for daemon registration and telemetry.
//!
//! Generates a SHA-256 fingerprint from a platform hardware ID on first run,
//! stores it in the `settings` table, and returns the same value on every
//! subsequent startup.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::storage::Storage;

const SETTING_KEY: &str = "daemon_id";

/// Returns the stable daemon identity string.
///
/// On first call it reads a platform hardware ID, hashes it with SHA-256,
/// stores the hex digest in the `settings` table, and returns it.
/// On every subsequent call it reads and returns the stored value.
pub async fn get_or_create(storage: &Storage) -> Result<String> {
    if let Some(id) = storage.get_setting(SETTING_KEY).await? {
        return Ok(id);
    }

    let raw = platform_hardware_id().context("failed to read platform hardware ID")?;
    let digest = hex_sha256(raw.trim());
    storage.set_setting(SETTING_KEY, &digest).await?;
    Ok(digest)
}

// ─── Platform hardware ID ────────────────────────────────────────────────────

/// Returns a raw platform-specific hardware identifier string.
///
/// The caller is responsible for trimming/hashing this value.
fn platform_hardware_id() -> Result<String> {
    #[cfg(target_os = "macos")]
    return macos_platform_uuid();

    #[cfg(target_os = "linux")]
    return linux_machine_id();

    #[cfg(target_os = "windows")]
    return windows_machine_guid();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return fallback_id();
}

#[cfg(target_os = "macos")]
fn macos_platform_uuid() -> Result<String> {
    // ioreg -rd1 -c IOPlatformExpertDevice  (no external crate needed)
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .context("ioreg command failed")?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.contains("IOPlatformUUID") {
            // line looks like: "IOPlatformUUID" = "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
            if let Some(start) = line.rfind('"') {
                let tail = &line[..start];
                if let Some(end) = tail.rfind('"') {
                    return Ok(line[end + 1..start].to_string());
                }
            }
        }
    }
    anyhow::bail!("IOPlatformUUID not found in ioreg output")
}

#[cfg(target_os = "linux")]
fn linux_machine_id() -> Result<String> {
    // /etc/machine-id is guaranteed on any systemd-based distro
    std::fs::read_to_string("/etc/machine-id")
        .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
        .context("no machine-id file found")
}

#[cfg(target_os = "windows")]
fn windows_machine_guid() -> Result<String> {
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .context("reg query failed")?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.contains("MachineGuid") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(guid) = parts.last() {
                return Ok(guid.to_string());
            }
        }
    }
    anyhow::bail!("MachineGuid not found in registry output")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn fallback_id() -> Result<String> {
    // Unsupported platform — use process start time as entropy seed.
    // This won't be stable across reboots, but prevents a hard failure.
    Ok(format!("fallback-{}", std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_nanos()))
}

// ─── Hashing ─────────────────────────────────────────────────────────────────

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_deterministic() {
        let a = hex_sha256("test-input");
        let b = hex_sha256("test-input");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // 32 bytes × 2 hex chars
    }

    #[test]
    fn sha256_different_inputs_differ() {
        assert_ne!(hex_sha256("a"), hex_sha256("b"));
    }
}
