// SPDX-License-Identifier: MIT
//! Project RPC handlers.

use crate::AppContext;
use anyhow::Result;
use serde_json::{json, Value};

use super::model::{CreateProjectParams, UpdateProjectParams};
use super::storage::ProjectStorage;

/// JSON-RPC error code: project not found.
pub const PROJECT_NOT_FOUND: i64 = -32023;
/// JSON-RPC error code: repository already in project.
pub const REPO_ALREADY_IN_PROJECT: i64 = -32024;

fn proj_storage(ctx: &AppContext) -> ProjectStorage {
    ProjectStorage::new(ctx.storage.pool())
}

// ─── Project handlers ─────────────────────────────────────────────────────────

/// `project.create` — create a new project.
pub async fn project_create(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params["name"].as_str().unwrap_or("").to_string();
    let root_path = params["rootPath"].as_str().map(str::to_string);
    let description = params["description"].as_str().map(str::to_string);
    let org_slug = params["orgSlug"].as_str().map(str::to_string);

    let project = proj_storage(ctx)
        .create(CreateProjectParams {
            name,
            root_path,
            description,
            org_slug,
        })
        .await?;

    ctx.broadcaster.broadcast(
        "project.created",
        json!({ "project": project }),
    );

    Ok(serde_json::to_value(&project)?)
}

/// `project.list` — list all projects.
pub async fn project_list(_params: Value, ctx: &AppContext) -> Result<Value> {
    let projects = proj_storage(ctx).list().await?;
    Ok(json!({ "projects": projects }))
}

/// `project.get` — get a project with its repos.
pub async fn project_get(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"].as_str().unwrap_or("").to_string();
    match proj_storage(ctx).get_with_repos(&id).await? {
        Some(with_repos) => Ok(serde_json::to_value(&with_repos)?),
        None => anyhow::bail!("PROJECT_NOT_FOUND: {}", id),
    }
}

/// `project.update` — update a project's metadata.
pub async fn project_update(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"].as_str().unwrap_or("").to_string();
    let name = params["name"].as_str().map(str::to_string);
    let description = params["description"].as_str().map(str::to_string);
    let org_slug = params["orgSlug"].as_str().map(str::to_string);

    let project = proj_storage(ctx)
        .update(&id, UpdateProjectParams { name, description, org_slug })
        .await?;

    ctx.broadcaster.broadcast(
        "project.updated",
        json!({ "project": project }),
    );

    Ok(serde_json::to_value(&project)?)
}

/// `project.delete` — delete a project (cascades to repos).
pub async fn project_delete(params: Value, ctx: &AppContext) -> Result<Value> {
    let id = params["id"].as_str().unwrap_or("").to_string();
    let existed = proj_storage(ctx).delete(&id).await?;
    if !existed {
        anyhow::bail!("PROJECT_NOT_FOUND: {}", id);
    }

    ctx.broadcaster.broadcast(
        "project.deleted",
        json!({ "id": id }),
    );

    Ok(json!({ "deleted": true, "id": id }))
}

/// `project.addRepo` — add a git repository to a project.
///
/// Validates that the path is a real git repository before inserting.
pub async fn project_add_repo(params: Value, ctx: &AppContext) -> Result<Value> {
    let project_id = params["projectId"].as_str().unwrap_or("").to_string();
    let repo_path = params["repoPath"].as_str().unwrap_or("").to_string();

    proj_storage(ctx).add_repo(&project_id, &repo_path).await?;

    ctx.broadcaster.broadcast(
        "project.repoAdded",
        json!({ "projectId": project_id, "repoPath": repo_path }),
    );

    Ok(json!({ "projectId": project_id, "repoPath": repo_path }))
}

/// `project.removeRepo` — remove a repository from a project.
pub async fn project_remove_repo(params: Value, ctx: &AppContext) -> Result<Value> {
    let project_id = params["projectId"].as_str().unwrap_or("").to_string();
    let repo_path = params["repoPath"].as_str().unwrap_or("").to_string();

    let removed = proj_storage(ctx).remove_repo(&project_id, &repo_path).await?;
    if !removed {
        anyhow::bail!("REPO_NOT_IN_PROJECT: {}", repo_path);
    }

    ctx.broadcaster.broadcast(
        "project.repoRemoved",
        json!({ "projectId": project_id, "repoPath": repo_path }),
    );

    Ok(json!({ "projectId": project_id, "repoPath": repo_path, "removed": true }))
}

/// `daemon.setName` — persist the human-readable name for this daemon host.
pub async fn daemon_set_name(params: Value, ctx: &AppContext) -> Result<Value> {
    let name = params["name"].as_str().unwrap_or("").to_string();
    proj_storage(ctx).set_host_name(&name).await?;
    Ok(json!({ "name": name }))
}
