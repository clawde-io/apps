// cli/license.rs — `clawd license verify` + `clawd license info` CLI commands.
//
// Sprint NN AG.3

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::license::bundle::LicenseBundle;

/// Verify a license bundle file and print its contents.
pub fn cmd_verify(bundle_path: PathBuf, daemon_id: &str) -> Result<()> {
    println!("Verifying license bundle: {}", bundle_path.display());
    let bundle = LicenseBundle::load_and_verify(&bundle_path, daemon_id)
        .context("License verification failed")?;

    println!("\n✓ License valid\n");
    println!("  Tier:      {}", bundle.tier);
    println!("  Daemon ID: {}", bundle.daemon_id);
    if let Some(seats) = bundle.seat_count {
        println!("  Seats:     {}", seats);
    }
    println!("  Issued:    {}", bundle.issued_at);
    println!("  Expires:   {}", bundle.expires_at);
    println!("\nFeatures:");
    println!("  relay:       {}", bundle.features.relay);
    println!("  auto_switch: {}", bundle.features.auto_switch);
    println!("  air_gap:     {}", bundle.features.air_gap);

    Ok(())
}

/// Print info about the currently loaded license (from daemon settings/cache).
pub fn cmd_info(license_path: Option<PathBuf>) -> Result<()> {
    if let Some(path) = license_path {
        if !path.exists() {
            println!("No license bundle found at {}", path.display());
            println!("Running in cloud-connected mode (online verification).");
            return Ok(());
        }
        // Parse bundle (no signature verify — info only)
        let content =
            std::fs::read_to_string(&path).with_context(|| format!("Reading {:?}", path))?;
        let lines: Vec<&str> = content.trim().lines().collect();
        if lines.is_empty() {
            println!("License file is empty.");
            return Ok(());
        }
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let payload_bytes = URL_SAFE_NO_PAD.decode(lines[0].trim()).unwrap_or_default();
        if let Ok(bundle) = serde_json::from_slice::<LicenseBundle>(&payload_bytes) {
            println!("License bundle (unverified metadata):");
            println!("  Tier:    {}", bundle.tier);
            println!("  Expires: {}", bundle.expires_at);
            println!("  Air-gap: {}", bundle.features.air_gap);
        } else {
            println!("Cannot parse license bundle payload.");
        }
    } else {
        println!("No license path configured. Running in cloud mode.");
        println!("Set [connectivity] license_path in ~/.claw/config.toml for air-gap mode.");
    }
    Ok(())
}
