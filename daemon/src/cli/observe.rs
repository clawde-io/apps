// cli/observe.rs — `clawd observe --session <id>` (Sprint ZZ OT.T06)
//
// Pretty-prints the OTel trace tree for a session from SQLite.
// No external OTel collector required for basic use.

use anyhow::Result;
use serde_json::json;
use std::path::Path;

/// OT.T06 — `clawd observe --session <id>`
pub async fn observe(session_id: String, data_dir: &Path, port: u16) -> Result<()> {
    let token = super::client::read_auth_token(data_dir)?;
    let client = super::client::DaemonClient::new(port, token);

    let result = client
        .call_once("session.trace", json!({ "session_id": session_id }))
        .await?;

    let spans = result["spans"].as_array().cloned().unwrap_or_default();

    if spans.is_empty() {
        println!("No trace data found for session {session_id}.");
        println!("(Traces are recorded when OTEL_EXPORTER_OTLP_ENDPOINT is set, or stored locally.)");
        return Ok(());
    }

    println!("Session trace: {session_id}");
    println!();

    // Build parent → children map
    let mut children: std::collections::HashMap<Option<String>, Vec<&serde_json::Value>> =
        std::collections::HashMap::new();
    for span in &spans {
        let parent_id = span["parent_span_id"].as_str().map(String::from);
        children.entry(parent_id).or_default().push(span);
    }

    // Print root spans
    if let Some(roots) = children.get(&None) {
        for root in roots {
            print_span_tree(root, &children, 0);
        }
    }

    Ok(())
}

fn print_span_tree(
    span: &serde_json::Value,
    children: &std::collections::HashMap<Option<String>, Vec<&serde_json::Value>>,
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    let name = span["name"].as_str().unwrap_or("?");
    let duration_ms = span["duration_ms"].as_u64();
    let status = span["status"].as_str().unwrap_or("ok");
    let status_icon = match status {
        "error" => "✗",
        "running" => "◌",
        _ => "✓",
    };

    let duration_str = match duration_ms {
        Some(ms) if ms >= 1000 => format!(" ({:.1}s)", ms as f64 / 1000.0),
        Some(ms) => format!(" ({ms}ms)"),
        None => String::new(),
    };

    println!("{indent}{status_icon} {name}{duration_str}");

    // Print key attributes (skip span_id, trace_id, parent_span_id)
    if let Some(attrs) = span["attributes"].as_object() {
        let skip_keys = ["span_id", "trace_id", "parent_span_id"];
        for (key, val) in attrs {
            if !skip_keys.contains(&key.as_str()) {
                let val_str = val.as_str().unwrap_or(&val.to_string());
                if !val_str.is_empty() && val_str != "null" {
                    println!("{indent}  ↳ {key}: {val_str}");
                }
            }
        }
    }

    // Recurse to children
    let span_id = span["span_id"].as_str().map(String::from);
    if let Some(child_spans) = children.get(&span_id) {
        for child in child_spans {
            print_span_tree(child, children, depth + 1);
        }
    }
}
