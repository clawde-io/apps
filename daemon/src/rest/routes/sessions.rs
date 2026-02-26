// rest/routes/sessions.rs — Session REST routes (Sprint QQ RA.2-3).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn list_sessions(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let sessions = ctx.session_manager.list_sessions().await;
    let list: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "status": format!("{:?}", s.status).to_lowercase(),
                "provider": s.provider,
                "repo_path": s.repo_path,
                "created_at": s.created_at,
            })
        })
        .collect();
    Json(json!({ "sessions": list }))
}

pub async fn get_session(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ctx.session_manager.get_session(&id).await {
        Some(s) => Ok(Json(json!({
            "id": s.id,
            "status": format!("{:?}", s.status).to_lowercase(),
            "provider": s.provider,
            "repo_path": s.repo_path,
            "created_at": s.created_at,
            "model_override": s.model_override,
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Session not found" })),
        )),
    }
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub provider: Option<String>,
    pub repo_path: Option<String>,
}

pub async fn create_session(
    State(ctx): State<Arc<AppContext>>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let params = json!({
        "provider": body.provider.unwrap_or_else(|| "claude".to_string()),
        "repo_path": body.repo_path.unwrap_or_default(),
    });

    match crate::ipc::handlers::session::create(params, ctx).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}

#[derive(Deserialize)]
pub struct SubmitTaskRequest {
    pub content: String,
}

pub async fn submit_task(
    State(ctx): State<Arc<AppContext>>,
    Path(session_id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let params = json!({
        "session_id": session_id,
        "content": body.content,
        "role": "user",
    });

    match crate::ipc::handlers::session::send_message(params, ctx).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )),
    }
}
