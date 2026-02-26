// SPDX-License-Identifier: MIT
// Sprint M: Pack Marketplace — IPC handler shims
//
// Thin wrappers that forward JSON-RPC dispatch calls to crate::packs::handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::Value;

pub async fn install(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_install(params, ctx).await
}

pub async fn update(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_update(params, ctx).await
}

pub async fn remove(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_remove(params, ctx).await
}

pub async fn search(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_search(params, ctx).await
}

pub async fn publish(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_publish(params, ctx).await
}

pub async fn list_installed(params: Value, ctx: &AppContext) -> Result<Value> {
    crate::packs::handlers::pack_list_installed(params, ctx).await
}
