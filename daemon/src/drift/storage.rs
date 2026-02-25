/// SQLite persistence for drift items.
use crate::drift::scanner::{DriftItem, DriftSeverity};
use anyhow::Result;
use sqlx::SqlitePool;

#[derive(Debug, sqlx::FromRow)]
struct DriftRow {
    id: String,
    feature: String,
    severity: String,
    kind: String,
    message: String,
    location: Option<String>,
    project_path: String,
    #[allow(dead_code)]
    resolved: i64,
    #[allow(dead_code)]
    detected_at: String,
}

impl From<DriftRow> for DriftItem {
    fn from(r: DriftRow) -> Self {
        DriftItem {
            id: r.id,
            feature: r.feature,
            severity: match r.severity.as_str() {
                "critical" => DriftSeverity::Critical,
                "high" => DriftSeverity::High,
                "low" => DriftSeverity::Low,
                _ => DriftSeverity::Medium,
            },
            kind: r.kind,
            message: r.message,
            location: r.location,
            project_path: r.project_path,
        }
    }
}

/// Upsert a batch of drift items for a project.
pub async fn upsert_items(pool: &SqlitePool, items: &[DriftItem]) -> Result<()> {
    for item in items {
        let sev = item.severity.as_str();
        sqlx::query(
            "INSERT OR REPLACE INTO drift_items \
             (id, feature, severity, kind, message, location, project_path, resolved) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&item.id)
        .bind(&item.feature)
        .bind(sev)
        .bind(&item.kind)
        .bind(&item.message)
        .bind(&item.location)
        .bind(&item.project_path)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Clear all unresolved drift items for a project before a fresh scan.
pub async fn clear_unresolved(pool: &SqlitePool, project_path: &str) -> Result<()> {
    sqlx::query("DELETE FROM drift_items WHERE project_path = ? AND resolved = 0")
        .bind(project_path)
        .execute(pool)
        .await?;
    Ok(())
}

/// List drift items for a project, optionally filtered by severity.
pub async fn list_items(
    pool: &SqlitePool,
    project_path: &str,
    severity_filter: Option<&str>,
) -> Result<Vec<DriftItem>> {
    let rows: Vec<DriftRow> = if let Some(sev) = severity_filter {
        sqlx::query_as(
            "SELECT id, feature, severity, kind, message, location, project_path, resolved, detected_at \
             FROM drift_items \
             WHERE project_path = ? AND severity = ? AND resolved = 0 \
             ORDER BY detected_at DESC",
        )
        .bind(project_path)
        .bind(sev)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, feature, severity, kind, message, location, project_path, resolved, detected_at \
             FROM drift_items \
             WHERE project_path = ? AND resolved = 0 \
             ORDER BY detected_at DESC",
        )
        .bind(project_path)
        .fetch_all(pool)
        .await?
    };

    Ok(rows.into_iter().map(DriftItem::from).collect())
}

/// Count unresolved drift items for a project.
pub async fn count_unresolved(pool: &SqlitePool, project_path: &str) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM drift_items WHERE project_path = ? AND resolved = 0",
    )
    .bind(project_path)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
