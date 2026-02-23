// SPDX-License-Identifier: MIT
//! Project SQLite operations.

use anyhow::Result;
use sqlx::SqlitePool;

use super::model::*;

pub struct ProjectStorage {
    pub(crate) pool: SqlitePool,
}

impl ProjectStorage {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ─── Projects ─────────────────────────────────────────────────────────────

    pub async fn create(&self, params: CreateProjectParams) -> Result<Project> {
        let id = new_id();
        let now = unixepoch();
        sqlx::query(
            "INSERT INTO projects (id, name, root_path, description, org_slug, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&params.name)
        .bind(&params.root_path)
        .bind(&params.description)
        .bind(&params.org_slug)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("project not found after insert"))
    }

    pub async fn list(&self) -> Result<Vec<Project>> {
        Ok(
            sqlx::query_as("SELECT * FROM projects ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn get(&self, id: &str) -> Result<Option<Project>> {
        Ok(
            sqlx::query_as("SELECT * FROM projects WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn update(&self, id: &str, params: UpdateProjectParams) -> Result<Project> {
        let now = unixepoch();
        // Build partial update — only set fields that were provided
        sqlx::query(
            "UPDATE projects SET \
             name = COALESCE(?, name), \
             description = COALESCE(?, description), \
             org_slug = COALESCE(?, org_slug), \
             updated_at = ? \
             WHERE id = ?",
        )
        .bind(&params.name)
        .bind(&params.description)
        .bind(&params.org_slug)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.get(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("PROJECT_NOT_FOUND: {}", id))
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(rows > 0)
    }

    // ─── Project Repos ────────────────────────────────────────────────────────

    /// Add a repository to a project.
    ///
    /// Validates that `repo_path` is a real git repository via `git2`.
    /// If the project has a `root_path`, also validates that `repo_path`
    /// is within that root using `safe_path`.
    pub async fn add_repo(&self, project_id: &str, repo_path: &str) -> Result<()> {
        // Validate it's a real git repo
        git2::Repository::open(repo_path)
            .map_err(|e| anyhow::anyhow!("not a git repository: {} — {}", repo_path, e))?;

        // If the project has a root_path, validate no path traversal
        if let Some(project) = self.get(project_id).await? {
            if let Some(root) = &project.root_path {
                let root_path = std::path::Path::new(root);
                let rel = std::path::Path::new(repo_path)
                    .strip_prefix(root_path)
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "repo_path {} is not under project root_path {}",
                            repo_path,
                            root
                        )
                    })?;
                crate::security::safe_path(root_path, rel)?;
            }
        } else {
            anyhow::bail!("PROJECT_NOT_FOUND: {}", project_id);
        }

        // Check for duplicate
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT project_id FROM project_repos WHERE project_id = ? AND repo_path = ?")
                .bind(project_id)
                .bind(repo_path)
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            anyhow::bail!("REPO_ALREADY_IN_PROJECT: {}", repo_path);
        }

        let now = unixepoch();
        sqlx::query(
            "INSERT INTO project_repos (project_id, repo_path, added_at) VALUES (?, ?, ?)",
        )
        .bind(project_id)
        .bind(repo_path)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_repo(&self, project_id: &str, repo_path: &str) -> Result<bool> {
        let rows = sqlx::query(
            "DELETE FROM project_repos WHERE project_id = ? AND repo_path = ?",
        )
        .bind(project_id)
        .bind(repo_path)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(rows > 0)
    }

    pub async fn get_with_repos(&self, id: &str) -> Result<Option<ProjectWithRepos>> {
        let project = match self.get(id).await? {
            Some(p) => p,
            None => return Ok(None),
        };
        let repos: Vec<ProjectRepo> =
            sqlx::query_as("SELECT * FROM project_repos WHERE project_id = ? ORDER BY added_at ASC")
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
        Ok(Some(ProjectWithRepos { project, repos }))
    }

    // ─── Host Settings ────────────────────────────────────────────────────────

    pub async fn get_host_name(&self) -> Result<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM host_settings WHERE key = 'host_name'")
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(v,)| v).unwrap_or_default())
    }

    pub async fn set_host_name(&self, name: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO host_settings (key, value) VALUES ('host_name', ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ─── Recent Repos ─────────────────────────────────────────────────────────

    pub async fn recent_repos(&self, limit: i64) -> Result<Vec<ProjectRepo>> {
        Ok(sqlx::query_as(
            "SELECT * FROM project_repos \
             WHERE last_opened_at IS NOT NULL \
             ORDER BY last_opened_at DESC \
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }
}

fn unixepoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    async fn make_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await.unwrap();
        // Run the migration SQL directly
        let migration = include_str!("../storage/migrations/008_projects.sql");
        for stmt in migration.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(&pool).await.unwrap();
            }
        }
        pool
    }

    fn storage(pool: SqlitePool) -> ProjectStorage {
        ProjectStorage::new(pool)
    }

    #[tokio::test]
    async fn test_create_project() {
        let s = storage(make_pool().await);
        let p = s
            .create(CreateProjectParams {
                name: "MyProject".to_string(),
                root_path: None,
                description: Some("A test project".to_string()),
                org_slug: Some("acme".to_string()),
            })
            .await
            .unwrap();
        assert_eq!(p.name, "MyProject");
        assert_eq!(p.description.as_deref(), Some("A test project"));
        assert_eq!(p.org_slug.as_deref(), Some("acme"));
        assert!(p.created_at > 0);
        assert_eq!(p.created_at, p.updated_at);
    }

    #[tokio::test]
    async fn test_list_projects() {
        let s = storage(make_pool().await);
        s.create(CreateProjectParams {
            name: "Alpha".to_string(),
            root_path: None,
            description: None,
            org_slug: None,
        })
        .await
        .unwrap();
        s.create(CreateProjectParams {
            name: "Beta".to_string(),
            root_path: None,
            description: None,
            org_slug: None,
        })
        .await
        .unwrap();
        let projects = s.list().await.unwrap();
        assert_eq!(projects.len(), 2);
    }

    #[tokio::test]
    async fn test_get_project() {
        let s = storage(make_pool().await);
        let created = s
            .create(CreateProjectParams {
                name: "GetMe".to_string(),
                root_path: None,
                description: None,
                org_slug: None,
            })
            .await
            .unwrap();
        let fetched = s.get(&created.id).await.unwrap().expect("should exist");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "GetMe");
    }

    #[tokio::test]
    async fn test_update_project() {
        let s = storage(make_pool().await);
        let created = s
            .create(CreateProjectParams {
                name: "Original".to_string(),
                root_path: None,
                description: None,
                org_slug: None,
            })
            .await
            .unwrap();
        let updated = s
            .update(
                &created.id,
                UpdateProjectParams {
                    name: Some("Updated".to_string()),
                    description: Some("New desc".to_string()),
                    org_slug: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description.as_deref(), Some("New desc"));
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn test_delete_project() {
        let s = storage(make_pool().await);
        let p = s
            .create(CreateProjectParams {
                name: "ToDelete".to_string(),
                root_path: None,
                description: None,
                org_slug: None,
            })
            .await
            .unwrap();
        let existed = s.delete(&p.id).await.unwrap();
        assert!(existed);
        let gone = s.get(&p.id).await.unwrap();
        assert!(gone.is_none());
        // Deleting again returns false
        let again = s.delete(&p.id).await.unwrap();
        assert!(!again);
    }

    #[tokio::test]
    async fn test_add_and_remove_repo() {
        let s = storage(make_pool().await);
        let p = s
            .create(CreateProjectParams {
                name: "RepoProject".to_string(),
                root_path: None,
                description: None,
                org_slug: None,
            })
            .await
            .unwrap();

        // Use a real git repo path — the daemon workspace itself
        let repo_path = "/Users/admin/Sites/clawde/apps";
        // Only run if the repo actually exists (local-only test)
        if !std::path::Path::new(repo_path).exists() {
            return;
        }
        if git2::Repository::open(repo_path).is_err() {
            return;
        }

        s.add_repo(&p.id, repo_path).await.unwrap();

        let with_repos = s.get_with_repos(&p.id).await.unwrap().unwrap();
        assert_eq!(with_repos.repos.len(), 1);
        assert_eq!(with_repos.repos[0].repo_path, repo_path);

        // Adding same repo again should fail
        let dup = s.add_repo(&p.id, repo_path).await;
        assert!(dup.is_err());
        assert!(dup.unwrap_err().to_string().contains("REPO_ALREADY_IN_PROJECT"));

        // Remove the repo
        let removed = s.remove_repo(&p.id, repo_path).await.unwrap();
        assert!(removed);

        let with_repos = s.get_with_repos(&p.id).await.unwrap().unwrap();
        assert!(with_repos.repos.is_empty());
    }

    #[tokio::test]
    async fn test_host_name() {
        let s = storage(make_pool().await);
        // Default is empty string
        let name = s.get_host_name().await.unwrap();
        assert_eq!(name, "");
        // Set and retrieve
        s.set_host_name("my-mac").await.unwrap();
        let name = s.get_host_name().await.unwrap();
        assert_eq!(name, "my-mac");
        // Overwrite
        s.set_host_name("other-name").await.unwrap();
        let name = s.get_host_name().await.unwrap();
        assert_eq!(name, "other-name");
    }
}
