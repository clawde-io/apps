//! Sprint DD WR.3 — `workflow.*` RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

/// `workflow.create` — store a new workflow recipe from YAML.
pub async fn create(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("name required"))?;
    let yaml = params
        .get("yaml")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("yaml required"))?;
    let description = params
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tags = params.get("tags").cloned().unwrap_or_else(|| json!([]));

    // Validate YAML parses correctly.
    crate::workflows::engine::parse_recipe_yaml(yaml)?;

    let id = Uuid::new_v4().to_string();
    let tags_json = serde_json::to_string(&tags)?;

    sqlx::query(
        "INSERT INTO workflow_recipes (id, name, description, template_yaml, tags)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(description)
    .bind(yaml)
    .bind(&tags_json)
    .execute(ctx.storage.pool())
    .await?;

    Ok(json!({ "id": id, "name": name }))
}

/// `workflow.list` — list all workflow recipes (built-ins + user-defined).
pub async fn list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let rows = sqlx::query(
        "SELECT id, name, description, tags, is_builtin, run_count, created_at
         FROM workflow_recipes ORDER BY is_builtin DESC, name ASC",
    )
    .fetch_all(ctx.storage.pool())
    .await?;

    use sqlx::Row as _;
    let recipes: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "name": r.get::<String, _>("name"),
                "description": r.get::<String, _>("description"),
                "tags": serde_json::from_str::<Value>(r.get::<String, _>("tags").as_str()).unwrap_or(json!([])),
                "isBuiltin": r.get::<bool, _>("is_builtin"),
                "runCount": r.get::<i64, _>("run_count"),
                "createdAt": r.get::<String, _>("created_at"),
            })
        })
        .collect();

    Ok(json!({ "recipes": recipes }))
}

/// `workflow.run` — start executing a workflow recipe.
///
/// Executes each step sequentially, creating a session per step and feeding
/// `inherit_from` for chained context. Pushes `workflow.stepCompleted` after
/// each step and `workflow.ran` when all steps complete.
pub async fn run(params: Value, ctx: &AppContext) -> Result<Value> {
    let recipe_id = params
        .get("recipeId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("recipeId required"))?;
    let repo_path = params
        .get("repoPath")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let inputs: std::collections::HashMap<String, String> = params
        .get("inputs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Load recipe YAML.
    use sqlx::Row as _;
    let row = sqlx::query(
        "SELECT id, name, template_yaml, is_builtin FROM workflow_recipes WHERE id = ?",
    )
    .bind(recipe_id)
    .fetch_optional(ctx.storage.pool())
    .await?
    .ok_or_else(|| anyhow::anyhow!("workflow recipe not found: {}", recipe_id))?;

    let recipe_yaml: String = row.get("template_yaml");
    let recipe_name: String = row.get("name");
    let recipe = crate::workflows::engine::parse_recipe_yaml(&recipe_yaml)?;

    let run_id = Uuid::new_v4().to_string();
    let total_steps = recipe.steps.len() as i64;

    sqlx::query(
        "INSERT INTO workflow_runs (id, recipe_id, status, total_steps) VALUES (?, ?, 'running', ?)",
    )
    .bind(&run_id)
    .bind(recipe_id)
    .bind(total_steps)
    .execute(ctx.storage.pool())
    .await?;

    // Increment run_count.
    sqlx::query("UPDATE workflow_recipes SET run_count = run_count + 1 WHERE id = ?")
        .bind(recipe_id)
        .execute(ctx.storage.pool())
        .await?;

    // Execute steps in a background task.
    let run_id_bg = run_id.clone();
    let recipe_id_bg = recipe_id.to_string();
    let recipe_name_bg = recipe_name.clone();
    let ctx_bg = ctx.clone();
    let repo_path_owned = repo_path.to_string();

    tokio::spawn(async move {
        let mut prev_session_id: Option<String> = None;

        for (i, step) in recipe.steps.iter().enumerate() {
            // Substitute {key} placeholders from inputs.
            let mut prompt = step.prompt.clone();
            for (k, v) in &inputs {
                prompt = prompt.replace(&format!("{{{}}}", k), v);
            }

            let inherit = if step.inherit_from.as_deref() == Some("previous") {
                prev_session_id.clone()
            } else {
                None
            };

            let provider = step.provider.as_deref().unwrap_or("claude");

            // Create session for this step.
            match ctx_bg
                .session_manager
                .create(
                    provider,
                    &repo_path_owned,
                    &format!("{} — step {}", recipe_name_bg, i + 1),
                    0,
                    None,
                    Some(&prompt),
                )
                .await
            {
                Ok(session) => {
                    if inherit.is_some() {
                        // Send prompt as first message so it runs immediately.
                        let _ = ctx_bg
                            .session_manager
                            .send_message(&session.id, &prompt, &ctx_bg)
                            .await;
                    }

                    prev_session_id = Some(session.id.clone());

                    // Update run state.
                    let _ = sqlx::query("UPDATE workflow_runs SET current_step = ? WHERE id = ?")
                        .bind(i as i64 + 1)
                        .bind(&run_id_bg)
                        .execute(ctx_bg.storage.pool())
                        .await;

                    ctx_bg.broadcaster.broadcast(
                        "workflow.stepCompleted",
                        json!({
                            "runId": run_id_bg,
                            "recipeId": recipe_id_bg,
                            "stepIndex": i,
                            "sessionId": session.id,
                        }),
                    );
                }
                Err(e) => {
                    let _ = sqlx::query(
                        "UPDATE workflow_runs SET status = 'failed', finished_at = datetime('now') WHERE id = ?",
                    )
                    .bind(&run_id_bg)
                    .execute(ctx_bg.storage.pool())
                    .await;

                    ctx_bg.broadcaster.broadcast(
                        "workflow.failed",
                        json!({ "runId": run_id_bg, "error": e.to_string() }),
                    );
                    return;
                }
            }
        }

        // Mark run complete.
        let _ = sqlx::query(
            "UPDATE workflow_runs SET status = 'done', finished_at = datetime('now') WHERE id = ?",
        )
        .bind(&run_id_bg)
        .execute(ctx_bg.storage.pool())
        .await;

        ctx_bg.broadcaster.broadcast(
            "workflow.ran",
            json!({
                "runId": run_id_bg,
                "recipeId": recipe_id_bg,
                "stepsCompleted": total_steps,
            }),
        );
    });

    Ok(json!({
        "runId": run_id,
        "recipeId": recipe_id,
        "recipeName": recipe_name,
        "totalSteps": total_steps,
        "status": "running",
    }))
}

/// `workflow.delete` — remove a user-defined workflow recipe.
pub async fn delete(params: Value, ctx: &AppContext) -> Result<Value> {
    let recipe_id = params
        .get("recipeId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("recipeId required"))?;

    sqlx::query("DELETE FROM workflow_recipes WHERE id = ? AND is_builtin = 0")
        .bind(recipe_id)
        .execute(ctx.storage.pool())
        .await?;

    Ok(json!({ "deleted": true }))
}
