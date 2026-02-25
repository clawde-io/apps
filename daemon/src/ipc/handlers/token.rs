// SPDX-License-Identifier: MIT
//! Token RPC handlers — `token.sessionUsage`, `token.totalUsage`, `token.budgetStatus`.
//!
//! These handlers expose the `TokenTracker` data to clients so Flutter and web
//! UI components can display per-session and monthly cost summaries.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

/// `token.sessionUsage` — return token + cost totals for one session.
///
/// Params: `{ "sessionId": "…" }`
///
/// Response:
/// ```json
/// {
///   "sessionId":         "abc123",
///   "inputTokens":       1234,
///   "outputTokens":      567,
///   "estimatedCostUsd":  0.012345,
///   "messageCount":      8
/// }
/// ```
pub async fn session_usage(params: Value, ctx: &AppContext) -> Result<Value> {
    let session_id = params
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sessionId required"))?;

    let usage = ctx.token_tracker.get_session_usage(session_id).await?;

    Ok(json!({
        "sessionId":        session_id,
        "inputTokens":      usage.input_tokens,
        "outputTokens":     usage.output_tokens,
        "estimatedCostUsd": usage.estimated_cost_usd,
        "messageCount":     usage.message_count,
    }))
}

/// `token.totalUsage` — return usage broken down by model over a date range.
///
/// Params (all optional):
/// ```json
/// { "from": "2026-02-01T00:00:00Z", "to": "2026-02-28T23:59:59Z" }
/// ```
///
/// Defaults to the current calendar month when `from`/`to` are omitted.
///
/// Response:
/// ```json
/// [
///   { "modelId": "claude-sonnet-4-6", "inputTokens": …, "outputTokens": …,
///     "estimatedCostUsd": …, "messageCount": … },
///   …
/// ]
/// ```
pub async fn total_usage(params: Value, ctx: &AppContext) -> Result<Value> {
    let from = params.get("from").and_then(|v| v.as_str());
    let to = params.get("to").and_then(|v| v.as_str());

    let breakdown = ctx.token_tracker.get_date_range_usage(from, to).await?;

    let rows: Vec<Value> = breakdown
        .into_iter()
        .map(|m| {
            json!({
                "modelId":          m.model_id,
                "inputTokens":      m.input_tokens,
                "outputTokens":     m.output_tokens,
                "estimatedCostUsd": m.estimated_cost_usd,
                "messageCount":     m.message_count,
            })
        })
        .collect();

    Ok(Value::Array(rows))
}

/// `token.budgetStatus` — return monthly spend vs optional budget cap.
///
/// Params: none required.  Optional: `{ "monthlyCap": 10.0 }` to check against
/// a specific cap (otherwise uses `model_intelligence.monthly_budget_usd` from
/// daemon config if set, or returns `null` for `cap` and `pct`).
///
/// Response:
/// ```json
/// {
///   "monthlySpendUsd": 3.14,
///   "cap":             10.0,   // null if no cap configured
///   "pct":             31.4,   // null if no cap configured
///   "warning":         false,  // true when pct >= 80
///   "exceeded":        false   // true when pct >= 100
/// }
/// ```
pub async fn budget_status(params: Value, ctx: &AppContext) -> Result<Value> {
    let monthly_spend = ctx.token_tracker.get_monthly_total().await?;

    // Prefer the cap from params, fall back to daemon config.
    let cap: Option<f64> = params
        .get("monthlyCap")
        .and_then(|v| v.as_f64())
        .or_else(|| {
            let budget = ctx.config.model_intelligence.monthly_budget_usd;
            if budget > 0.0 { Some(budget) } else { None }
        });

    let (cap_val, pct_val, warning, exceeded) = match cap {
        Some(c) if c > 0.0 => {
            let pct = (monthly_spend / c * 100.0).min(9999.9);
            (
                Value::from(c),
                Value::from(pct),
                pct >= 80.0,
                pct >= 100.0,
            )
        }
        _ => (Value::Null, Value::Null, false, false),
    };

    Ok(json!({
        "monthlySpendUsd": monthly_spend,
        "cap":             cap_val,
        "pct":             pct_val,
        "warning":         warning,
        "exceeded":        exceeded,
    }))
}
