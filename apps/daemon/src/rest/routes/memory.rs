// rest/routes/memory.rs — GET /api/v1/memory (Sprint QQ RA.5).

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

#[derive(Deserialize)]
pub struct MemoryQuery {
    pub scope: Option<String>,
}

pub async fn list_memory(
    State(ctx): State<Arc<AppContext>>,
    Query(q): Query<MemoryQuery>,
) -> Json<Value> {
    let scope = q.scope.as_deref().unwrap_or("global");
    match ctx.memory_store.list(scope).await {
        Ok(entries) => {
            let list: Vec<Value> = entries
                .iter()
                .map(|e| {
                    json!({
                        "id": e.id,
                        "scope": e.scope,
                        "key": e.key,
                        "value": e.value,
                        "weight": e.weight,
                        "source": e.source,
                    })
                })
                .collect();
            Json(json!({ "entries": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
