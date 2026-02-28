// instructions/proposals.rs — Instruction proposal system (Sprint ZZ IL.T05/T06)
//
// After `review.run` completes, pattern-match repeated review findings across 3+ reviews.
// When a pattern is detected, propose a new instruction node with user approval required.

use crate::storage::Storage;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct InstructionProposal {
    pub id: String,
    pub from_review_ids: Vec<String>,
    pub suggested_scope: String,
    pub suggested_content: String,
    pub confidence: f64, // 0.0–1.0
    pub recurrence_count: u32,
    pub status: ProposalStatus,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Accepted,
    Dismissed,
}

pub struct ProposalEngine<'a> {
    storage: &'a Storage,
}

impl<'a> ProposalEngine<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// Scan recent review findings and generate proposals for repeated patterns.
    /// Only generates proposals for patterns seen in 3+ distinct reviews.
    pub async fn scan_and_propose(&self, project_path: &str) -> Result<Vec<String>> {
        let _ = project_path; // Used for path context in future versions

        // Query most common review findings across sessions
        let findings: Vec<ReviewFinding> = sqlx::query_as::<_, ReviewFinding>(
            "SELECT finding_text, COUNT(DISTINCT session_id) as count
             FROM review_findings
             WHERE created_at > datetime('now', '-30 days')
             GROUP BY finding_text
             HAVING count >= 3
             ORDER BY count DESC
             LIMIT 10",
        )
        .fetch_all(self.storage.pool())
        .await
        .unwrap_or_default();

        let mut proposal_ids = Vec::new();
        for finding in findings {
            let confidence = (finding.count as f64 / 10.0).min(0.95);
            let proposal_id = self.create_proposal(&finding, confidence).await?;
            proposal_ids.push(proposal_id);
        }

        Ok(proposal_ids)
    }

    async fn create_proposal(&self, finding: &ReviewFinding, confidence: f64) -> Result<String> {
        let id = format!("{}", uuid::Uuid::new_v4()).replace('-', "");
        let suggested_content = format!(
            "## Auto-proposed rule (from {} review findings)\n\n{}\n",
            finding.count,
            finding.finding_text.trim()
        );

        sqlx::query(
            "INSERT INTO instruction_proposals (id, suggested_scope, suggested_content, confidence, recurrence_count, status, created_at)
             VALUES (?, 'project', ?, ?, ?, 'pending', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        )
        .bind(&id)
        .bind(&suggested_content)
        .bind(confidence)
        .bind(finding.count)
        .execute(self.storage.pool())
        .await
        .context("insert instruction proposal")?;

        Ok(id)
    }

    /// Accept a proposal: create the instruction node + mark proposal accepted.
    pub async fn accept(&self, proposal_id: &str) -> Result<String> {
        let proposal = self.get_proposal(proposal_id).await?;

        // Create the instruction node
        let node_id = format!("{}", uuid::Uuid::new_v4()).replace('-', "");
        sqlx::query(
            "INSERT INTO instruction_nodes (id, scope, priority, owner, content_md)
             VALUES (?, ?, 100, 'auto-proposed', ?)",
        )
        .bind(&node_id)
        .bind(&proposal.suggested_scope)
        .bind(&proposal.suggested_content)
        .execute(self.storage.pool())
        .await
        .context("insert accepted instruction node")?;

        // Mark proposal accepted
        sqlx::query("UPDATE instruction_proposals SET status = 'accepted' WHERE id = ?")
            .bind(proposal_id)
            .execute(self.storage.pool())
            .await?;

        Ok(node_id)
    }

    /// Dismiss a proposal.
    pub async fn dismiss(&self, proposal_id: &str) -> Result<()> {
        sqlx::query("UPDATE instruction_proposals SET status = 'dismissed' WHERE id = ?")
            .bind(proposal_id)
            .execute(self.storage.pool())
            .await?;
        Ok(())
    }

    async fn get_proposal(&self, id: &str) -> Result<InstructionProposal> {
        let row = sqlx::query_as::<_, ProposalRow>(
            "SELECT id, suggested_scope, suggested_content, confidence, recurrence_count, status, created_at
             FROM instruction_proposals WHERE id = ?",
        )
        .bind(id)
        .fetch_one(self.storage.pool())
        .await
        .context("get proposal")?;

        Ok(InstructionProposal {
            id: row.id,
            from_review_ids: vec![],
            suggested_scope: row.suggested_scope,
            suggested_content: row.suggested_content,
            confidence: row.confidence,
            recurrence_count: row.recurrence_count as u32,
            status: match row.status.as_str() {
                "accepted" => ProposalStatus::Accepted,
                "dismissed" => ProposalStatus::Dismissed,
                _ => ProposalStatus::Pending,
            },
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ReviewFinding {
    pub finding_text: String,
    pub count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ProposalRow {
    pub id: String,
    pub suggested_scope: String,
    pub suggested_content: String,
    pub confidence: f64,
    pub recurrence_count: i64,
    pub status: String,
    pub created_at: String,
}
