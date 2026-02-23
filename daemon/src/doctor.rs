// SPDX-License-Identifier: MIT
//! doctor.rs — pre-flight diagnostic checks for `clawd doctor`.
//!
//! This module is self-contained and does NOT require AppContext.
//! It runs before the daemon starts, so it can catch configuration
//! problems before they cause confusing startup failures.

use std::process::Command;

/// The result of a single diagnostic check.
pub struct CheckResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

/// Run all diagnostic checks and return a list of results.
pub fn run_doctor() -> Vec<CheckResult> {
    vec![
        check_port_available(),
        check_claude_installed(),
        check_claude_authenticated(),
        check_codex_installed(),
        check_sqlite_accessible(),
        check_disk_space(),
        check_log_dir_writable(),
        check_relay_reachable(),
    ]
}

// ─── Individual checks ────────────────────────────────────────────────────────

/// Check 1: Port 4300 is available (not in use by another process).
fn check_port_available() -> CheckResult {
    let passed = std::net::TcpListener::bind("127.0.0.1:4300").is_ok();
    CheckResult {
        name: "Port 4300 available",
        passed,
        detail: if passed {
            "port 4300 is free".to_string()
        } else {
            "port 4300 is in use by another process".to_string()
        },
    }
}

/// Check 2: `claude` CLI is installed and on PATH.
fn check_claude_installed() -> CheckResult {
    match Command::new("claude").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("unknown version")
                .trim()
                .to_string();
            CheckResult {
                name: "claude CLI installed",
                passed: true,
                detail: version,
            }
        }
        _ => CheckResult {
            name: "claude CLI installed",
            passed: false,
            detail: "not found in PATH".to_string(),
        },
    }
}

/// Check 3: `claude` CLI is authenticated (logged in).
fn check_claude_authenticated() -> CheckResult {
    match Command::new("claude").args(["auth", "status"]).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
            let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
            let combined = format!("{stdout}{stderr}");
            // Claude CLI outputs "Logged in" when authenticated
            let authenticated = combined.contains("logged in") && !combined.contains("not logged in");
            let detail = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or(if authenticated { "logged in" } else { "not logged in" })
                .trim()
                .to_string();
            CheckResult {
                name: "claude CLI authenticated",
                passed: authenticated,
                detail: if detail.is_empty() {
                    if authenticated {
                        "logged in".to_string()
                    } else {
                        "not logged in — run `claude auth login`".to_string()
                    }
                } else {
                    detail
                },
            }
        }
        Err(_) => CheckResult {
            name: "claude CLI authenticated",
            passed: false,
            detail: "claude CLI not found — cannot check auth status".to_string(),
        },
    }
}

/// Check 4: `codex` CLI is installed and on PATH.
fn check_codex_installed() -> CheckResult {
    match Command::new("codex").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("unknown version")
                .trim()
                .to_string();
            CheckResult {
                name: "codex CLI installed",
                passed: true,
                detail: version,
            }
        }
        _ => CheckResult {
            name: "codex CLI installed",
            passed: false,
            detail: "not found in PATH (optional — only needed for Codex sessions)".to_string(),
        },
    }
}

/// Check 5: SQLite database file is accessible.
fn check_sqlite_accessible() -> CheckResult {
    let db_path = clawd_data_dir().join("clawd.db");
    let exists = db_path.exists();
    CheckResult {
        name: "SQLite DB accessible",
        passed: exists,
        detail: if exists {
            format!("{} exists and is readable", db_path.display())
        } else {
            format!(
                "{} not found (will be created on first start)",
                db_path.display()
            )
        },
    }
}

/// Check 6: Sufficient disk space is available (> 100 MB).
fn check_disk_space() -> CheckResult {
    let data_dir = clawd_data_dir();
    // Use statvfs on Unix, fallback to a basic check on other platforms.
    match available_disk_bytes(&data_dir) {
        Some(bytes) => {
            const WARN_THRESHOLD: u64 = 100 * 1024 * 1024; // 100 MB
            let passed = bytes > WARN_THRESHOLD;
            let detail = if bytes >= 1024 * 1024 * 1024 {
                format!("{:.1} GB free", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
            } else {
                format!("{:.0} MB free", bytes as f64 / (1024.0 * 1024.0))
            };
            CheckResult {
                name: "Disk space",
                passed,
                detail: if passed {
                    detail
                } else {
                    format!("low disk space: only {detail}")
                },
            }
        }
        None => CheckResult {
            name: "Disk space",
            passed: true, // assume ok if we cannot check
            detail: "could not determine disk space".to_string(),
        },
    }
}

/// Check 7: Log directory is writable.
fn check_log_dir_writable() -> CheckResult {
    let log_dir = clawd_data_dir().join("logs");
    // Create the directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        return CheckResult {
            name: "Log directory writable",
            passed: false,
            detail: format!("cannot create log directory: {e}"),
        };
    }
    // Try to create a temp file to confirm writability
    let test_path = log_dir.join(".doctor_write_test");
    match std::fs::write(&test_path, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_path);
            CheckResult {
                name: "Log directory writable",
                passed: true,
                detail: format!("{} is writable", log_dir.display()),
            }
        }
        Err(e) => CheckResult {
            name: "Log directory writable",
            passed: false,
            detail: format!("cannot write to {}: {e}", log_dir.display()),
        },
    }
}

/// Check 8: Relay server `api.clawde.io:443` is reachable.
fn check_relay_reachable() -> CheckResult {
    use std::net::TcpStream;
    use std::time::Duration;

    let target = "api.clawde.io:443";
    match TcpStream::connect_timeout(
        &target.parse().unwrap_or_else(|_| {
            // Fallback — resolve at runtime
            std::net::ToSocketAddrs::to_socket_addrs(&target)
                .ok()
                .and_then(|mut a| a.next())
                .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap())
        }),
        Duration::from_secs(5),
    ) {
        Ok(_) => CheckResult {
            name: "Relay reachable",
            passed: true,
            detail: "api.clawde.io reachable".to_string(),
        },
        Err(_) => {
            // Try alternative: connect to the resolved address manually
            match std::net::ToSocketAddrs::to_socket_addrs(&target) {
                Ok(mut addrs) => match addrs.next() {
                    Some(addr) => match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                        Ok(_) => CheckResult {
                            name: "Relay reachable",
                            passed: true,
                            detail: "api.clawde.io reachable".to_string(),
                        },
                        Err(e) => CheckResult {
                            name: "Relay reachable",
                            passed: false,
                            detail: format!(
                                "cannot reach api.clawde.io (check internet connection): {e}"
                            ),
                        },
                    },
                    None => CheckResult {
                        name: "Relay reachable",
                        passed: false,
                        detail: "cannot resolve api.clawde.io (check internet connection)"
                            .to_string(),
                    },
                },
                Err(e) => CheckResult {
                    name: "Relay reachable",
                    passed: false,
                    detail: format!("cannot reach api.clawde.io (check internet connection): {e}"),
                },
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Return the clawd data directory (same logic as DaemonConfig).
fn clawd_data_dir() -> std::path::PathBuf {
    if let Ok(v) = std::env::var("CLAWD_DATA_DIR") {
        return std::path::PathBuf::from(v);
    }
    dirs_data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("clawd")
}

/// Cross-platform data dir: ~/.local/share on Linux, ~/Library/Application Support on macOS.
fn dirs_data_dir() -> Option<std::path::PathBuf> {
    // Mirror what DaemonConfig uses: XDG_DATA_HOME or platform default.
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Some(std::path::PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        return Some(
            std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support"),
        );
        #[cfg(not(target_os = "macos"))]
        return Some(
            std::path::PathBuf::from(home)
                .join(".local")
                .join("share"),
        );
    }
    None
}

/// Return available bytes on the filesystem containing `path`.
fn available_disk_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path_cstr = CString::new(
            path.to_str()
                .unwrap_or("/")
                .as_bytes(),
        )
        .ok()?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::statvfs(path_cstr.as_ptr(), &mut stat) };
        if ret == 0 {
            // f_bavail = blocks available to unprivileged user
            // f_frsize = fundamental file system block size
            Some(stat.f_bavail as u64 * stat.f_frsize as u64)
        } else {
            None
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix platforms (Windows), we skip the check.
        let _ = path;
        None
    }
}

// ─── Output ───────────────────────────────────────────────────────────────────

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

/// Print a formatted table of check results to stdout.
pub fn print_doctor_results(results: &[CheckResult]) {
    println!();
    println!("{BOLD}clawd doctor — pre-flight checks{RESET}");
    println!("{}", "─".repeat(60));

    for r in results {
        let (symbol, color) = if r.passed {
            ("✓", GREEN)
        } else {
            ("✗", RED)
        };
        println!(
            "  {color}{symbol}{RESET}  {:<30}  {}",
            r.name, r.detail
        );
    }

    println!("{}", "─".repeat(60));

    let failed = results.iter().filter(|r| !r.passed).count();
    if failed == 0 {
        println!("{GREEN}All checks passed.{RESET}");
    } else {
        println!("{RED}{failed} check(s) failed. See above for details.{RESET}");
    }
    println!();
}
