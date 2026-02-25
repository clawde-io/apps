// SPDX-License-Identifier: MIT
//! doctor — AFS project health scanner (D64, Phase 64) + pre-flight CLI checks.
//!
//! Two distinct responsibilities:
//!
//! 1. **Pre-flight CLI checks** (`run_doctor` / `print_doctor_results`):
//!    Run before the daemon starts to catch config problems early.
//!    Self-contained, no AppContext required.
//!
//! 2. **AFS project scanner** (`scan` / `fix` / `approve_release`):
//!    Provides `doctor.scan`, `doctor.fix`, and `doctor.approveRelease` RPCs.
//!    All checks are stateless (filesystem scan only — no DB).

pub mod afs_checks;
pub mod docs_checks;
pub mod release_checks;
pub mod version_watcher;

use std::path::Path;
use std::process::Command;

// ═══════════════════════════════════════════════════════════════════════════════
// AFS scan/fix types and orchestration (D64)
// ═══════════════════════════════════════════════════════════════════════════════

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl DoctorSeverity {
    /// Score penalty for severity (subtracted from 100).
    pub fn penalty(&self) -> i32 {
        match self {
            DoctorSeverity::Critical => 20,
            DoctorSeverity::High => 10,
            DoctorSeverity::Medium => 5,
            DoctorSeverity::Low => 2,
            DoctorSeverity::Info => 0,
        }
    }
}

/// A single diagnostic finding from `doctor.scan`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DoctorFinding {
    /// Short code identifying the check (e.g. `afs.missing_vision`).
    pub code: String,
    pub severity: DoctorSeverity,
    pub message: String,
    /// Path relative to project root that the finding relates to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether `doctor.fix` can automatically resolve this finding.
    pub fixable: bool,
}

/// Full result of a `doctor.scan` call.
#[derive(Debug, serde::Serialize)]
pub struct DoctorScanResult {
    /// Overall health score: 100 = perfect, 0 = critical issues.
    pub score: u8,
    pub findings: Vec<DoctorFinding>,
}

/// Result of `doctor.fix`.
#[derive(Debug, serde::Serialize)]
pub struct DoctorFixResult {
    pub fixed: Vec<String>,
    pub skipped: Vec<String>,
}

/// Scope for `doctor.scan`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanScope {
    All,
    Afs,
    Docs,
    Release,
}

impl ScanScope {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "afs" => ScanScope::Afs,
            "docs" => ScanScope::Docs,
            "release" => ScanScope::Release,
            _ => ScanScope::All,
        }
    }
}

/// Run all enabled checks and return a scored result.
pub fn scan(project_path: &Path, scope: ScanScope) -> DoctorScanResult {
    let mut findings: Vec<DoctorFinding> = Vec::new();

    if scope == ScanScope::All || scope == ScanScope::Afs {
        findings.extend(afs_checks::run(project_path));
    }
    if scope == ScanScope::All || scope == ScanScope::Docs {
        findings.extend(docs_checks::run(project_path));
    }
    if scope == ScanScope::All || scope == ScanScope::Release {
        findings.extend(release_checks::run(project_path));
    }

    let penalty: i32 = findings.iter().map(|f| f.severity.penalty()).sum();
    let score = (100_i32 - penalty).clamp(0, 100) as u8;

    DoctorScanResult { score, findings }
}

/// Apply auto-fixable repairs for the specified finding codes.
/// Passing an empty `codes` slice fixes ALL fixable findings.
pub fn fix(project_path: &Path, codes: &[String]) -> DoctorFixResult {
    let scan_result = scan(project_path, ScanScope::All);
    let mut fixed: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for finding in &scan_result.findings {
        let matches = codes.is_empty() || codes.contains(&finding.code);
        if !matches {
            continue;
        }
        if !finding.fixable {
            skipped.push(finding.code.clone());
            continue;
        }
        let ok = apply_fix(project_path, &finding.code);
        if ok {
            fixed.push(finding.code.clone());
        } else {
            skipped.push(finding.code.clone());
        }
    }

    DoctorFixResult { fixed, skipped }
}

/// Apply a single auto-fix identified by its code.
fn apply_fix(project_path: &Path, code: &str) -> bool {
    match code {
        "afs.missing_gitignore_entry" => {
            // Add .claude/ to .gitignore
            let gitignore = project_path.join(".gitignore");
            let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
            let updated = format!("{}\n# AI agent directory\n.claude/\n", content.trim_end());
            std::fs::write(&gitignore, updated).is_ok()
        }
        "afs.stale_temp" => {
            // Remove temp/ files older than 24h
            let temp = project_path.join(".claude/temp");
            if !temp.exists() {
                return true;
            }
            let cutoff = std::time::SystemTime::now()
                .checked_sub(std::time::Duration::from_secs(24 * 60 * 60))
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let mut all_ok = true;
            if let Ok(entries) = std::fs::read_dir(&temp) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified < cutoff && std::fs::remove_file(entry.path()).is_err() {
                                all_ok = false;
                            }
                        }
                    }
                }
            }
            all_ok
        }
        "afs.missing_ideas_dir" => {
            std::fs::create_dir_all(project_path.join(".claude/ideas")).is_ok()
        }
        "docs.missing_docs_readme" => {
            let docs = project_path.join(".docs");
            if !docs.exists() && std::fs::create_dir_all(&docs).is_err() {
                return false;
            }
            let readme = docs.join("README.md");
            if readme.exists() {
                return true;
            }
            std::fs::write(
                &readme,
                "# Project Docs\n\nThis directory contains project documentation.\n",
            )
            .is_ok()
        }
        _ => false,
    }
}

/// Update the release plan status to `approved`.
/// Returns true if the file was found and updated.
pub fn approve_release(project_path: &Path, version: &str) -> bool {
    let plan_path = project_path
        .join(".claude/planning")
        .join(format!("release-{version}.md"));

    let content = match std::fs::read_to_string(&plan_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Update "Status: draft" → "Status: approved" (case-insensitive)
    let updated = if content.contains("Status: draft") || content.contains("Status: Draft") {
        content
            .replace("Status: draft", "Status: approved")
            .replace("Status: Draft", "Status: approved")
    } else if !content.contains("Status: approved") {
        // Append status if not present
        format!("{}\n\n## Status\n\napproved\n", content.trim_end())
    } else {
        return true; // already approved
    };

    std::fs::write(&plan_path, updated).is_ok()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pre-flight CLI checks (existing — used by `clawd doctor` subcommand)
// ═══════════════════════════════════════════════════════════════════════════════

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
            let authenticated =
                combined.contains("logged in") && !combined.contains("not logged in");
            let detail = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or(if authenticated {
                    "logged in"
                } else {
                    "not logged in"
                })
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
        return Some(std::path::PathBuf::from(home).join(".local").join("share"));
    }
    None
}

/// Return available bytes on the filesystem containing `path`.
fn available_disk_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path_cstr = CString::new(path.to_str().unwrap_or("/").as_bytes()).ok()?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::statvfs(path_cstr.as_ptr(), &mut stat) };
        if ret == 0 {
            // f_bavail = blocks available to unprivileged user
            // f_frsize = fundamental file system block size
            Some(stat.f_bavail as u64 * stat.f_frsize)
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
        println!("  {color}{symbol}{RESET}  {:<30}  {}", r.name, r.detail);
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
