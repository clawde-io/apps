//! Criterion benchmarks for hot paths in the clawd daemon.
//!
//! Run with:
//!   cargo bench
//!
//! Covers:
//!   - JSON-RPC request parsing (serde_json)
//!   - Secret redaction (regex pipeline)
//!   - Rate-limiter check_and_record (HashMap + Instant)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::Value;

// ─── JSON-RPC parsing ────────────────────────────────────────────────────────

static SESSION_SEND_MSG: &str = r#"{
    "jsonrpc": "2.0",
    "id": 42,
    "method": "session.sendMessage",
    "params": {
        "sessionId": "01HXYZ1234567890ABCDEFGHIJ",
        "message": "Implement the new feature and add tests for edge cases."
    }
}"#;

static DAEMON_STATUS: &str = r#"{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "daemon.status",
    "params": {}
}"#;

fn bench_rpc_parse(c: &mut Criterion) {
    c.bench_function("rpc_parse_session_sendMessage", |b| {
        b.iter(|| {
            let v: Value = serde_json::from_str(black_box(SESSION_SEND_MSG)).unwrap();
            black_box(v);
        });
    });

    c.bench_function("rpc_parse_daemon_status", |b| {
        b.iter(|| {
            let v: Value = serde_json::from_str(black_box(DAEMON_STATUS)).unwrap();
            black_box(v);
        });
    });

    c.bench_function("rpc_serialize_response", |b| {
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "version": "0.1.0",
                "uptime": 12345,
                "activeSessions": 3,
                "watchedRepos": 2
            }
        });
        b.iter(|| {
            let s = serde_json::to_string(black_box(&resp)).unwrap();
            black_box(s);
        });
    });
}

// ─── Secret redaction ────────────────────────────────────────────────────────
//
// The redactor replaces common secret patterns (API keys, tokens, passwords) in
// text with a placeholder. This runs on every tool output line.

use once_cell::sync::Lazy;
use regex::Regex;

static SECRET_PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        (
            Regex::new(r"(?i)(api[_-]?key|apikey)\s*[:=]\s*\S+").unwrap(),
            "[REDACTED API KEY]",
        ),
        (
            Regex::new(r"(?i)(token|access[_-]?token|auth[_-]?token)\s*[:=]\s*\S+").unwrap(),
            "[REDACTED TOKEN]",
        ),
        (
            Regex::new(r"(?i)(password|passwd|secret)\s*[:=]\s*\S+").unwrap(),
            "[REDACTED SECRET]",
        ),
        (
            Regex::new(r"Bearer [A-Za-z0-9\-._~+/]+=*").unwrap(),
            "[REDACTED BEARER]",
        ),
    ]
});

fn redact_secrets(input: &str) -> String {
    let mut out = input.to_owned();
    for (re, replacement) in SECRET_PATTERNS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

fn bench_secret_redaction(c: &mut Criterion) {
    let clean_line = "Writing file src/session.rs with 200 lines of Rust code.";
    let dirty_line = "Error: API_KEY=sk-abcdef12345 Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig password=hunter2";
    let long_clean = "a".repeat(4096);

    c.bench_function("redact_clean_line", |b| {
        b.iter(|| {
            let r = redact_secrets(black_box(clean_line));
            black_box(r);
        });
    });

    c.bench_function("redact_dirty_line", |b| {
        b.iter(|| {
            let r = redact_secrets(black_box(dirty_line));
            black_box(r);
        });
    });

    c.bench_function("redact_long_clean_4k", |b| {
        b.iter(|| {
            let r = redact_secrets(black_box(&long_clean));
            black_box(r);
        });
    });
}

// ─── Rate limiter ────────────────────────────────────────────────────────────
//
// Simulates the per-IP connection rate limiter: HashMap<IpAddr, Vec<Instant>>.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;

struct BenchLimiter {
    map: HashMap<IpAddr, Vec<Instant>>,
    window_secs: u64,
    limit: usize,
}

impl BenchLimiter {
    fn new(window_secs: u64, limit: usize) -> Self {
        Self {
            map: HashMap::new(),
            window_secs,
            limit,
        }
    }

    fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(self.window_secs);
        let entries = self.map.entry(ip).or_default();
        entries.retain(|t| now.duration_since(*t) < window);
        if entries.len() >= self.limit {
            return false;
        }
        entries.push(now);
        true
    }
}

fn bench_rate_limiter(c: &mut Criterion) {
    let ip: IpAddr = "192.168.1.100".parse().unwrap();

    c.bench_function("rate_limiter_allow", |b| {
        // Fresh limiter each iteration — always allows
        b.iter_with_setup(
            || BenchLimiter::new(60, 100),
            |mut limiter| {
                black_box(limiter.check_and_record(black_box(ip)));
            },
        );
    });

    c.bench_function("rate_limiter_10_ips", |b| {
        let ips: Vec<IpAddr> = (1u8..=10)
            .map(|i| format!("10.0.0.{i}").parse().unwrap())
            .collect();
        b.iter_with_setup(
            || BenchLimiter::new(60, 100),
            |mut limiter| {
                for ip in &ips {
                    black_box(limiter.check_and_record(black_box(*ip)));
                }
            },
        );
    });
}

// ─── Entry point ─────────────────────────────────────────────────────────────

criterion_group!(benches, bench_rpc_parse, bench_secret_redaction, bench_rate_limiter);
criterion_main!(benches);
