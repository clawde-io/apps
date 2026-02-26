// rest/auth.rs — Bearer token auth middleware (Sprint QQ RA.6).
//
// Token is stored in `~/.claw/config.toml` under `[api] token = "..."`.
// Generate with: `clawd api-token generate`
// Header: Authorization: Bearer <token>

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use std::sync::Arc;

use crate::AppContext;

pub async fn require_api_auth(
    State(ctx): State<Arc<AppContext>>,
    req: Request,
    next: Next,
) -> Response {
    // Extract Bearer token from Authorization header
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let expected = ctx.config.api_token.as_deref().unwrap_or("");

    if expected.is_empty() {
        // Auth disabled — allow all (not recommended in production)
        return next.run(req).await;
    }

    match token {
        Some(t) if t == expected => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid or missing API token" })),
        )
            .into_response(),
    }
}
