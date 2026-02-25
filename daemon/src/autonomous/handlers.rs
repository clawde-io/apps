// SPDX-License-Identifier: MIT
//! JSON-RPC handlers for the Autonomous Execution Engine (Sprint J, AE.T01–AE.T20).
//!
//! Registered methods (add to `ipc/mod.rs` dispatch table — see sprint_J_wiring_notes.md):
//!   ae.plan.create      — generate an AePlan for a message (AE.T01/T03)
//!   ae.plan.approve     — mark a plan approved (AE.T02)
//!   ae.plan.get         — retrieve a plan by ID (AE.T03)
//!   ae.decision.record  — persist a session decision (AE.T06)
//!   ae.confidence.get   — compute + return confidence score (AE.T12)
//!   recipe.list         — list registered workflow recipes (AE.T19)
//!   recipe.create       — register a new workflow recipe (AE.T19)

use crate::autonomous::{
    confidence::ConfidenceScorer, plan_generator::PlanGenerator, recipe::WorkflowRecipe,
    AePlan,
};
use crate::AppContext;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

// ─── Param structs ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PlanCreateParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    message: String,
}

#[derive(Deserialize)]
struct PlanIdParams {
    #[serde(rename = "planId")]
    plan_id: String,
}

#[derive(Deserialize)]
struct RecordDecisionParams {
    #[serde(rename = "sessionId")]
    session_id: String,
    description: String,
    #[serde(default)]
    context: Value,
}

#[derive(Deserialize)]
struct ConfidenceGetParams {
    #[serde(rename = "planId")]
    plan_id: String,
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Deserialize)]
struct RecipeCreateParams {
    id: String,
    name: String,
    #[serde(rename = "triggerPattern", default)]
    trigger_pattern: String,
    steps: Vec<Value>,
}

// ─── ae.plan.create ──────────────────────────────────────────────────────────

/// Generate an AePlan heuristically from a user message and persist it.
///
/// The plan is stored in `ae_plans`.  The daemon emits an `ae.planReady`
/// push event after storage so the Flutter UI can display the preview card.
pub async fn ae_plan_create(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: PlanCreateParams = serde_json::from_value(params)?;

    if p.session_id.trim().is_empty() {
        bail!("INVALID_PARAMS: sessionId must not be empty");
    }
    if p.message.trim().is_empty() {
        bail!("INVALID_PARAMS: message must not be empty");
    }

    // Verify the session exists.
    ctx.storage
        .get_session(&p.session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("SESSION_NOT_FOUND"))?;

    let plan = PlanGenerator::generate_plan(&p.message, &p.session_id)?;
    persist_plan(ctx, &plan).await?;

    // Emit push event so Flutter plan-preview card appears.
    ctx.broadcaster.broadcast(
        "ae.planReady",
        plan_to_json(&plan),
    );

    Ok(plan_to_json(&plan))
}

// ─── ae.plan.approve ─────────────────────────────────────────────────────────

/// Mark a plan as approved by the user.  Updates `approved_at` timestamp.
pub async fn ae_plan_approve(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: PlanIdParams = serde_json::from_value(params)?;

    let approved_at = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE ae_plans SET approved_at = ? WHERE id = ?")
        .bind(&approved_at)
        .bind(&p.plan_id)
        .execute(&ctx.storage.pool())
        .await?;

    ctx.broadcaster.broadcast(
        "ae.planApproved",
        json!({ "planId": p.plan_id, "approvedAt": approved_at }),
    );

    Ok(json!({ "planId": p.plan_id, "approvedAt": approved_at }))
}

// ─── ae.plan.get ─────────────────────────────────────────────────────────────

/// Retrieve a stored plan by ID.
pub async fn ae_plan_get(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: PlanIdParams = serde_json::from_value(params)?;

    let row = sqlx::query(
        "SELECT id, session_id, title, requirements, definition_of_done, \
         files_expected, ai_instructions, created_at, approved_at, parent_task_id \
         FROM ae_plans WHERE id = ?",
    )
    .bind(&p.plan_id)
    .fetch_optional(&ctx.storage.pool())
    .await?;

    match row {
        None => bail!("ae_plan not found: {}", p.plan_id),
        Some(r) => {
            use sqlx::Row;
            Ok(json!({
                "id":               r.get::<String, _>("id"),
                "sessionId":        r.get::<String, _>("session_id"),
                "title":            r.get::<String, _>("title"),
                "requirements":     serde_json::from_str::<Value>(&r.get::<String, _>("requirements")).unwrap_or(json!([])),
                "definitionOfDone": serde_json::from_str::<Value>(&r.get::<String, _>("definition_of_done")).unwrap_or(json!([])),
                "filesExpected":    serde_json::from_str::<Value>(&r.get::<String, _>("files_expected")).unwrap_or(json!([])),
                "aiInstructions":   r.get::<Option<String>, _>("ai_instructions"),
                "createdAt":        r.get::<String, _>("created_at"),
                "approvedAt":       r.get::<Option<String>, _>("approved_at"),
                "parentTaskId":     r.get::<Option<String>, _>("parent_task_id"),
            }))
        }
    }
}

// ─── ae.decision.record ──────────────────────────────────────────────────────

/// Persist a key decision made during the session (AE.T06).
pub async fn ae_decision_record(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: RecordDecisionParams = serde_json::from_value(params)?;

    if p.description.trim().is_empty() {
        bail!("INVALID_PARAMS: description must not be empty");
    }

    let id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let context_str = p.context.to_string();

    sqlx::query(
        "INSERT INTO ae_decisions (id, session_id, description, context, created_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&p.session_id)
    .bind(&p.description)
    .bind(&context_str)
    .bind(&created_at)
    .execute(&ctx.storage.pool())
    .await?;

    Ok(json!({
        "id":          id,
        "sessionId":   p.session_id,
        "description": p.description,
        "createdAt":   created_at,
    }))
}

// ─── ae.confidence.get ───────────────────────────────────────────────────────

/// Compute and return the confidence score for a plan.
///
/// Fetches the plan and recent assistant messages, then runs the heuristic scorer.
pub async fn ae_confidence_get(params: Value, ctx: &AppContext) -> Result<Value> {
    let p: ConfidenceGetParams = serde_json::from_value(params)?;

    // Load the plan.
    let plan_row = sqlx::query(
        "SELECT id, session_id, title, requirements, definition_of_done, \
         files_expected, ai_instructions, created_at, approved_at \
         FROM ae_plans WHERE id = ?",
    )
    .bind(&p.plan_id)
    .fetch_optional(&ctx.storage.pool())
    .await?;

    let plan_row = plan_row.ok_or_else(|| anyhow::anyhow!("ae_plan not found: {}", p.plan_id))?;

    use sqlx::Row;
    let requirements: Vec<String> = serde_json::from_str(plan_row.get::<&str, _>("requirements")).unwrap_or_default();
    let dod: Vec<String> = serde_json::from_str(plan_row.get::<&str, _>("definition_of_done")).unwrap_or_default();
    let files: Vec<std::path::PathBuf> = serde_json::from_str::<Vec<String>>(plan_row.get::<&str, _>("files_expected"))
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();

    let plan = AePlan {
        id: plan_row.get::<String, _>("id"),
        session_id: plan_row.get::<String, _>("session_id"),
        title: plan_row.get::<String, _>("title"),
        requirements,
        definition_of_done: dod,
        files_expected: files,
        ai_instructions: plan_row.get::<Option<String>, _>("ai_instructions").unwrap_or_default(),
        created_at: plan_row.get::<String, _>("created_at"),
        approved_at: plan_row.get::<Option<String>, _>("approved_at"),
        parent_task_id: None,
    };

    // Fetch recent assistant message contents for confidence scoring.
    let messages: Vec<String> = sqlx::query(
        "SELECT content FROM messages \
         WHERE session_id = ? AND role = 'assistant' \
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(&p.session_id)
    .fetch_all(&ctx.storage.pool())
    .await?
    .into_iter()
    .filter_map(|r| r.try_get::<Option<String>, _>("content").ok().flatten())
    .collect();

    let confidence = ConfidenceScorer::build_task_confidence(&plan, &messages);

    Ok(json!({
        "planId":  confidence.task_id,
        "score":   confidence.score,
        "signals": confidence.signals.iter().map(|s| json!({
            "name":    s.name,
            "present": s.present,
            "weight":  s.weight,
        })).collect::<Vec<_>>(),
    }))
}

// ─── recipe.list ─────────────────────────────────────────────────────────────

/// List all registered workflow recipes stored in the daemon's config dir.
///
/// The list is assembled from the `.clawd/recipes/` directory of the active
/// repo.  If no repo is active, falls back to the global recipes dir.
pub async fn recipe_list(_params: Value, _ctx: &AppContext) -> Result<Value> {
    // Stub: returns an empty list.  Full implementation wires to the RecipeEngine
    // stored in AppContext (future sprint — see sprint_J_wiring_notes.md).
    Ok(json!({ "recipes": serde_json::Value::Array(vec![]) }))
}

// ─── recipe.create ───────────────────────────────────────────────────────────

/// Register a new workflow recipe.
pub async fn recipe_create(params: Value, _ctx: &AppContext) -> Result<Value> {
    let p: RecipeCreateParams = serde_json::from_value(params)?;

    if p.id.trim().is_empty() {
        bail!("INVALID_PARAMS: id must not be empty");
    }
    if p.name.trim().is_empty() {
        bail!("INVALID_PARAMS: name must not be empty");
    }

    let steps: Vec<crate::autonomous::recipe::RecipeStep> = p
        .steps
        .into_iter()
        .filter_map(|step| {
            let action = step["action"].as_str()?.to_owned();
            let params: std::collections::HashMap<String, String> = step["params"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            Some(crate::autonomous::recipe::RecipeStep { action, params })
        })
        .collect();

    let recipe = WorkflowRecipe {
        id: p.id.clone(),
        name: p.name.clone(),
        trigger_pattern: p.trigger_pattern.clone(),
        steps,
    };

    // Stub: the RecipeEngine is not yet wired into AppContext.
    // Wiring is documented in sprint_J_wiring_notes.md.
    let _ = recipe;

    Ok(json!({
        "id":             p.id,
        "name":           p.name,
        "triggerPattern": p.trigger_pattern,
    }))
}

// ─── Persistence helpers ──────────────────────────────────────────────────────

async fn persist_plan(ctx: &AppContext, plan: &AePlan) -> Result<()> {
    let requirements = serde_json::to_string(&plan.requirements)?;
    let dod = serde_json::to_string(&plan.definition_of_done)?;
    let files: Vec<String> = plan
        .files_expected
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let files_str = serde_json::to_string(&files)?;

    sqlx::query(
        "INSERT OR REPLACE INTO ae_plans \
         (id, session_id, title, requirements, definition_of_done, \
          files_expected, ai_instructions, created_at, approved_at, parent_task_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&plan.id)
    .bind(&plan.session_id)
    .bind(&plan.title)
    .bind(&requirements)
    .bind(&dod)
    .bind(&files_str)
    .bind(&plan.ai_instructions)
    .bind(&plan.created_at)
    .bind(&plan.approved_at)
    .bind(&plan.parent_task_id)
    .execute(&ctx.storage.pool())
    .await?;

    Ok(())
}

fn plan_to_json(plan: &AePlan) -> Value {
    let files: Vec<String> = plan
        .files_expected
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    json!({
        "id":               plan.id,
        "sessionId":        plan.session_id,
        "title":            plan.title,
        "requirements":     plan.requirements,
        "definitionOfDone": plan.definition_of_done,
        "filesExpected":    files,
        "aiInstructions":   plan.ai_instructions,
        "createdAt":        plan.created_at,
        "approvedAt":       plan.approved_at,
        "parentTaskId":     plan.parent_task_id,
    })
}
