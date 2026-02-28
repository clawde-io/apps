// SPDX-License-Identifier: MIT
//! Project data model types.

use serde::{Deserialize, Serialize};

/// Generate a new ULID string.
pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}

/// A project is a named container for one or more git repositories.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: Option<String>,
    pub description: Option<String>,
    pub org_slug: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_active_at: Option<i64>,
}

/// A git repository associated with a project.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectRepo {
    pub project_id: String,
    pub repo_path: String,
    pub added_at: i64,
    pub last_opened_at: Option<i64>,
}

/// A project with its associated repositories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithRepos {
    pub project: Project,
    pub repos: Vec<ProjectRepo>,
}

/// Parameters for creating a new project.
#[derive(Debug, Deserialize)]
pub struct CreateProjectParams {
    pub name: String,
    pub root_path: Option<String>,
    pub description: Option<String>,
    pub org_slug: Option<String>,
}

/// Parameters for updating an existing project.
#[derive(Debug, Deserialize)]
pub struct UpdateProjectParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub org_slug: Option<String>,
}
