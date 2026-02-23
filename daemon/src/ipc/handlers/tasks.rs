use crate::tasks::{
    markdown_parser,
    queue_serializer,
    storage::{ActivityQueryParams, TaskListParams, TASK_NOT_FOUND},
};
use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|v| v.as_str()).map(String::from)
}
fn sv<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}
fn n(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|v| v.as_i64())
}

pub async fn list(params: Value, ctx: &AppContext) -> Result<Value> {
    let query = TaskListParams {
        repo_path: s(&params, "repo_path"),
        status: s(&params, "status"),
        agent: s(&params, "agent"),
        severity: s(&params, "severity"),
        phase: s(&params, "phase"),
        tag: s(&params, "tag"),
        search: s(&params, "search"),
        limit: n(&params, "limit"),
        offset: n(&params, "offset"),
    };
    let tasks = ctx.task_storage.list_tasks(&query).await?;
    Ok(json!({ "tasks": tasks }))
}

pub async fn get(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = sv(&params, "task_id")
        .ok_or_else(|| anyhow::anyhow!("TASK_CODE:{}", TASK_NOT_FOUND))?;
    let task = ctx.task_storage.get_task(id).await?
        .ok_or_else(|| anyhow::anyhow!("TASK_CODE:{}", TASK_NOT_FOUND))?;
    Ok(json!({ "task": task }))
}

pub async fn claim(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing task_id"))?;
    let agent_id = sv(&params, "agent_id").ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;

    let existing = ctx.task_storage.get_task(task_id).await?;
    let is_resume = existing.as_ref().map(|t| t.status == "interrupted").unwrap_or(false);

    let task = ctx.task_storage.claim_task(task_id, agent_id).await?;
    let _ = ctx.task_storage.set_agent_current_task(agent_id, Some(task_id)).await;

    let (action, detail) = if is_resume {
        ("session_resume", "Resumed from interrupted state.".to_string())
    } else {
        ("task_claimed", "pending → in_progress".to_string())
    };

    let _ = ctx.task_storage.log_activity(
        agent_id, Some(task_id), task.phase.as_deref(),
        action, "system", Some(&detail), None, &task.repo_path,
    ).await;

    let event = if is_resume { "task.resumed" } else { "task.claimed" };
    ctx.broadcaster.broadcast(event, json!({
        "task_id": task_id,
        "agent_id": agent_id,
        "is_resume": is_resume,
    }));

    let _ = queue_serializer::flush_queue(&ctx.task_storage, &task.repo_path).await;
    Ok(json!({ "task": task, "is_resume": is_resume }))
}

pub async fn release(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing task_id"))?;
    let agent_id = sv(&params, "agent_id").ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let task = ctx.task_storage.get_task(task_id).await?
        .ok_or_else(|| anyhow::anyhow!("TASK_CODE:{}", TASK_NOT_FOUND))?;

    ctx.task_storage.release_task(task_id, agent_id).await?;
    let _ = ctx.task_storage.set_agent_current_task(agent_id, None).await;
    let _ = queue_serializer::flush_queue(&ctx.task_storage, &task.repo_path).await;

    ctx.broadcaster.broadcast("task.released", json!({ "task_id": task_id }));
    Ok(json!({ "ok": true }))
}

pub async fn heartbeat(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing task_id"))?;
    let agent_id = sv(&params, "agent_id").ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    ctx.task_storage.heartbeat_task(task_id, agent_id).await?;
    ctx.task_storage.update_agent_heartbeat(agent_id).await?;
    Ok(json!({ "ok": true }))
}

pub async fn update_status(params: Value, ctx: &AppContext) -> Result<Value> {
    let task_id = sv(&params, "task_id").ok_or_else(|| anyhow::anyhow!("missing task_id"))?;
    let new_status = sv(&params, "status").ok_or_else(|| anyhow::anyhow!("missing status"))?;
    let notes = sv(&params, "notes");
    let block_reason = sv(&params, "block_reason");
    let agent_id = sv(&params, "agent_id").unwrap_or("system");

    let task = ctx.task_storage.update_status(task_id, new_status, notes, block_reason).await?;

    let _ = ctx.task_storage.log_activity(
        agent_id, Some(task_id), task.phase.as_deref(),
        "status_transition", "system",
        Some(&format!("→ {}", new_status)),
        None, &task.repo_path,
    ).await;

    ctx.broadcaster.broadcast("task.statusChanged", json!({
        "task_id": task_id,
        "status": new_status,
    }));

    let _ = queue_serializer::flush_queue(&ctx.task_storage, &task.repo_path).await;
    Ok(json!({ "task": task }))
}

pub async fn add_task(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = sv(&params, "id").ok_or_else(|| anyhow::anyhow!("missing id"))?;
    let title = sv(&params, "title").ok_or_else(|| anyhow::anyhow!("missing title"))?;
    let repo_path = sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing repo_path"))?;

    let files_str = params.get("files").map(|v| v.to_string());
    let depends_str = params.get("depends_on").map(|v| v.to_string());
    let tags_str = params.get("tags").map(|v| v.to_string());

    let task = ctx.task_storage.add_task(
        id, title,
        sv(&params, "type"),
        sv(&params, "phase"),
        sv(&params, "group"),
        sv(&params, "parent_id"),
        sv(&params, "severity"),
        sv(&params, "file"),
        files_str.as_deref(),
        depends_str.as_deref(),
        tags_str.as_deref(),
        n(&params, "estimated_minutes"),
        repo_path,
    ).await?;

    ctx.broadcaster.broadcast("task.created", json!({ "task_id": id }));
    let _ = queue_serializer::flush_queue(&ctx.task_storage, repo_path).await;
    Ok(json!({ "task": task }))
}

pub async fn bulk_add(params: Value, ctx: &AppContext) -> Result<Value> {
    let tasks_arr = params.get("tasks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing tasks array"))?
        .clone();
    let repo_path = sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing repo_path"))?.to_string();

    let mut created = 0usize;
    for t in &tasks_arr {
        let id = t.get("id").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("task missing id"))?;
        let title = t.get("title").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("task missing title"))?;
        ctx.task_storage.add_task(
            id, title,
            t.get("type").and_then(|v| v.as_str()),
            t.get("phase").and_then(|v| v.as_str()),
            t.get("group").and_then(|v| v.as_str()),
            t.get("parent_id").and_then(|v| v.as_str()),
            t.get("severity").and_then(|v| v.as_str()),
            t.get("file").and_then(|v| v.as_str()),
            None, None, None,
            t.get("estimated_minutes").and_then(|v| v.as_i64()),
            &repo_path,
        ).await?;
        created += 1;
    }

    let _ = queue_serializer::flush_queue(&ctx.task_storage, &repo_path).await;
    Ok(json!({ "created": created }))
}

pub async fn log_activity(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent = sv(&params, "agent_id").ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let action = sv(&params, "action").ok_or_else(|| anyhow::anyhow!("missing action"))?;
    let repo_path = sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing repo_path"))?;
    let meta_str = params.get("meta").map(|v| v.to_string());

    let entry = ctx.task_storage.log_activity(
        agent,
        sv(&params, "task_id"),
        sv(&params, "phase"),
        action,
        sv(&params, "entry_type").unwrap_or("auto"),
        sv(&params, "detail"),
        meta_str.as_deref(),
        repo_path,
    ).await?;

    ctx.broadcaster.broadcast("task.activityLogged", serde_json::to_value(&entry)?);
    Ok(json!({ "ok": true, "id": entry.id }))
}

pub async fn note(params: Value, ctx: &AppContext) -> Result<Value> {
    let agent = sv(&params, "agent_id").ok_or_else(|| anyhow::anyhow!("missing agent_id"))?;
    let note_text = sv(&params, "note").ok_or_else(|| anyhow::anyhow!("missing note"))?;
    let repo_path = sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing repo_path"))?;
    let note_text = if note_text.len() > 2000 { &note_text[..2000] } else { note_text };

    let entry = ctx.task_storage.post_note(
        agent,
        sv(&params, "task_id"),
        sv(&params, "phase"),
        note_text,
        repo_path,
    ).await?;

    ctx.broadcaster.broadcast("task.activityLogged", serde_json::to_value(&entry)?);
    Ok(json!({ "ok": true, "id": entry.id }))
}

pub async fn activity(params: Value, ctx: &AppContext) -> Result<Value> {
    let query = ActivityQueryParams {
        repo_path: s(&params, "repo_path"),
        task_id: s(&params, "task_id"),
        agent: s(&params, "agent"),
        phase: s(&params, "phase"),
        entry_type: s(&params, "entry_type"),
        action: s(&params, "action"),
        since: n(&params, "since"),
        limit: n(&params, "limit"),
        offset: n(&params, "offset"),
    };

    let entries = ctx.task_storage.query_activity(&query).await?;
    Ok(json!({ "entries": entries, "count": entries.len() }))
}

pub async fn from_planning(params: Value, ctx: &AppContext) -> Result<Value> {
    let path = sv(&params, "path").ok_or_else(|| anyhow::anyhow!("missing path"))?;
    let repo_path = sv(&params, "repo_path").ok_or_else(|| anyhow::anyhow!("missing repo_path"))?;

    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| anyhow::anyhow!("cannot read planning doc: {e}"))?;
    let parsed = markdown_parser::parse_active_md(&content);
    let count = ctx.task_storage.backfill_from_tasks(parsed, repo_path).await?;
    let _ = queue_serializer::flush_queue(&ctx.task_storage, repo_path).await;

    Ok(json!({ "imported": count }))
}

pub async fn from_checklist(params: Value, ctx: &AppContext) -> Result<Value> {
    from_planning(params, ctx).await
}

pub async fn summary(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = sv(&params, "repo_path");
    ctx.task_storage.summary(repo_path).await
}

pub async fn export(params: Value, ctx: &AppContext) -> Result<Value> {
    let format = sv(&params, "format").unwrap_or("json");
    let repo_path = sv(&params, "repo_path");

    let tasks = ctx.task_storage.list_tasks(&TaskListParams {
        repo_path: repo_path.map(String::from),
        ..Default::default()
    }).await?;

    match format {
        "csv" => {
            let mut csv = "id,title,type,phase,severity,status,claimed_by,file,notes\n".to_string();
            for t in &tasks {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    csv_escape(&t.id),
                    csv_escape(&t.title),
                    csv_escape(t.task_type.as_deref().unwrap_or("")),
                    csv_escape(t.phase.as_deref().unwrap_or("")),
                    csv_escape(t.severity.as_deref().unwrap_or("")),
                    t.status,
                    csv_escape(t.claimed_by.as_deref().unwrap_or("")),
                    csv_escape(t.file.as_deref().unwrap_or("")),
                    csv_escape(t.notes.as_deref().unwrap_or("")),
                ));
            }
            Ok(json!({ "format": "csv", "data": csv }))
        }
        _ => Ok(json!({ "format": "json", "tasks": tasks })),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn validate(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = sv(&params, "repo_path").unwrap_or("");
    let active_md_path = format!("{}/.claude/tasks/active.md", repo_path);
    let content = tokio::fs::read_to_string(&active_md_path).await.unwrap_or_default();
    let md_tasks = markdown_parser::parse_active_md(&content);

    let db_tasks = ctx.task_storage.list_tasks(&TaskListParams {
        repo_path: if repo_path.is_empty() { None } else { Some(repo_path.to_string()) },
        ..Default::default()
    }).await?;

    let md_ids: std::collections::HashSet<String> = md_tasks.iter().map(|t| t.id.clone()).collect();
    let db_ids: std::collections::HashSet<String> = db_tasks.iter().map(|t| t.id.clone()).collect();

    let only_in_md: Vec<&String> = md_ids.difference(&db_ids).collect();
    let only_in_db: Vec<&String> = db_ids.difference(&md_ids).collect();

    let mismatches: Vec<Value> = md_tasks.iter()
        .filter_map(|md| {
            db_tasks.iter().find(|db| db.id == md.id).and_then(|db| {
                if db.status != md.status {
                    Some(json!({ "id": md.id, "md": md.status, "db": db.status }))
                } else { None }
            })
        }).collect();

    Ok(json!({
        "only_in_markdown": only_in_md,
        "only_in_db": only_in_db,
        "status_mismatches": mismatches,
        "ok": only_in_md.is_empty() && only_in_db.is_empty() && mismatches.is_empty()
    }))
}

pub async fn sync(params: Value, ctx: &AppContext) -> Result<Value> {
    let repo_path = sv(&params, "repo_path").unwrap_or("");
    let active_md_path = format!("{}/.claude/tasks/active.md", repo_path);
    let content = tokio::fs::read_to_string(&active_md_path).await.unwrap_or_default();
    let parsed = markdown_parser::parse_active_md(&content);
    let count = ctx.task_storage.backfill_from_tasks(parsed, repo_path).await?;
    let _ = queue_serializer::flush_queue(&ctx.task_storage, repo_path).await;
    Ok(json!({ "synced": count }))
}
