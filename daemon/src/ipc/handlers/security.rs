// ipc/handlers/security.rs — Security RPC handlers (Sprint ZZ PI.T03, PI.T04)
//
// RPCs:
//   security.analyzeContent(content, source_type) → ContentAnalysis
//   security.testInjection() → EvalReport

use crate::security::content_labels::{
    analyze_content, record_content_label, sanitize_content, SourceType,
};
use crate::security::injection_eval::run_injection_eval;
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};


/// PI.T03 — `security.analyzeContent(content, source_type)` RPC
pub async fn analyze_content_rpc(ctx: &AppContext, params: Value) -> Result<Value> {
    let content = params["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing content"))?;
    let source_type_str = params["source_type"].as_str().unwrap_or("file");
    let session_id = params["session_id"].as_str().unwrap_or("unknown");

    let source_type = SourceType::from_str(source_type_str);
    let analysis = analyze_content(content, &source_type);

    // PI.T04 — For high-risk content: sanitize + log security event
    let sanitized = if analysis.risk_level == crate::security::content_labels::RiskLevel::High {
        let (sanitized_content, _stripped) = sanitize_content(content, &analysis);

        // Log security.injectionAttempt to audit_log
        let event_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let now = chrono::Utc::now().timestamp();
        let patterns_json = serde_json::to_string(&analysis.patterns_found)?;

        let _ = sqlx::query(
            "INSERT INTO audit_log \
             (id, actor_id, action, resource_type, resource_id, metadata_json, created_at) \
             VALUES (?, 'daemon', 'security.injectionAttempt', 'content', ?, ?, ?)",
        )
        .bind(&event_id)
        .bind(session_id)
        .bind(&patterns_json)
        .bind(now)
        .execute(ctx.storage.pool())
        .await;

        // Record content label
        let _ = record_content_label(&ctx.storage, session_id, &source_type, &analysis).await;

        // Push `security.contentSanitized` event to clients
        ctx.broadcaster.broadcast("security.contentSanitized", json!({
            "session_id": session_id,
            "patterns_found": &analysis.patterns_found,
            "source_type": source_type_str,
        }));

        Some(sanitized_content)
    } else {
        // Record label for tracking even if not high risk
        let _ = record_content_label(&ctx.storage, session_id, &source_type, &analysis).await;
        None
    };

    Ok(json!({
        "risk_level": analysis.risk_level.as_str(),
        "patterns_found": analysis.patterns_found,
        "is_untrusted_source": source_type.is_untrusted(),
        "sanitized_content": sanitized,
    }))
}

/// `security.testInjection()` RPC — run 20 red-team injection scenarios.
pub async fn test_injection(_ctx: &AppContext, _params: Value) -> Result<Value> {
    let (results, detection_rate) = run_injection_eval();

    let cases: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "id": r.scenario_id,
                "name": r.name,
                "expected": format!("{:?}", r.expected).to_lowercase(),
                "detected": format!("{:?}", r.detected).to_lowercase(),
                "passed": r.passed,
                "patterns_found": r.patterns_found,
            })
        })
        .collect();

    let passed = results.iter().filter(|r| r.passed).count();
    let total = results.len();

    Ok(json!({
        "detection_rate_pct": detection_rate,
        "total": total,
        "passed": passed,
        "failed": total - passed,
        "target_pct": 90.0,
        "meets_target": detection_rate >= 90.0,
        "cases": cases,
    }))
}
