// rest/mod.rs — Public REST API server (Sprint QQ RA.1).
//
// Axum HTTP server on port 4301 (local only unless relay is enabled).
// Bridges REST calls to the internal JSON-RPC handlers.
//
// Endpoints:
//   GET  /api/v1/sessions
//   POST /api/v1/sessions
//   GET  /api/v1/sessions/{id}
//   POST /api/v1/sessions/{id}/tasks
//   GET  /api/v1/sessions/{id}/events   (SSE)
//   GET  /api/v1/metrics
//   GET  /api/v1/memory
//   GET  /api/v1/openapi.json
//   GET  /api/v1/health

pub mod auth;
pub mod openapi;
pub mod routes;
pub mod sse;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

use crate::AppContext;

pub const REST_PORT: u16 = 4301;

pub async fn start_rest_server(ctx: Arc<AppContext>) -> Result<()> {
    let bind = format!("127.0.0.1:{REST_PORT}");
    let addr: SocketAddr = bind.parse()?;

    let router = build_router(ctx);

    info!("REST API listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

pub fn build_router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        // Health (no auth)
        .route("/api/v1/health", get(routes::health::health))
        // OpenAPI spec (no auth)
        .route("/api/v1/openapi.json", get(openapi::openapi_spec))
        // Sessions
        .route(
            "/api/v1/sessions",
            get(routes::sessions::list_sessions).post(routes::sessions::create_session),
        )
        .route("/api/v1/sessions/:id", get(routes::sessions::get_session))
        .route(
            "/api/v1/sessions/:id/tasks",
            post(routes::sessions::submit_task),
        )
        .route("/api/v1/sessions/:id/events", get(sse::session_events_sse))
        // Metrics
        .route("/api/v1/metrics", get(routes::metrics::get_metrics))
        // Memory
        .route("/api/v1/memory", get(routes::memory::list_memory))
        .with_state(ctx)
}
