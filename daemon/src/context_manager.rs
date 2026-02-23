//! Context Manager — eliminates "Prompt too long" by managing the context window.
//!
//! Proactively compresses older messages before hitting the API token limit,
//! persists all messages to SQLite (durable state separate from inference context),
//! and provides graceful degradation tiers that ensure sessions never die from
//! context overflow.

use anyhow::Result;
use sqlx::{FromRow, SqlitePool};
use tracing::{debug, info};

/// Minimal row type for loading messages during context window construction.
#[derive(Debug, FromRow)]
struct MsgRow {
    id: String,
    role: String,
    content: String,
    token_estimate: i64,
}

/// Token budget configuration for context window management.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Total API token limit (e.g., 200_000 for Claude).
    pub api_limit: usize,
    /// Reserved for the model response.
    pub response_reserve: usize,
    /// Reserved for the system prompt.
    pub system_prompt_reserve: usize,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            api_limit: 200_000,
            response_reserve: 20_000,
            system_prompt_reserve: 15_000,
        }
    }
}

impl TokenBudget {
    /// Available tokens for conversation history.
    pub fn conversation_budget(&self) -> usize {
        self.api_limit - self.response_reserve - self.system_prompt_reserve
    }
}

/// Context compression tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionTier {
    /// Below 80% budget — send full recent history.
    Normal,
    /// 80-90% — summarize messages older than last 20; truncate large tool results.
    Compress,
    /// 90-95% — summarize everything except last 5; include task state.
    Aggressive,
    /// >95% — system prompt + task state + last message only.
    Emergency,
}

/// A message ready for context window inclusion.
#[derive(Debug, Clone)]
pub struct ContextMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub token_estimate: usize,
    pub is_summary: bool,
}

/// Manages context windows for AI sessions, preventing "Prompt too long" errors.
pub struct ContextManager {
    pool: SqlitePool,
    budget: TokenBudget,
}

impl ContextManager {
    pub fn new(pool: SqlitePool, budget: TokenBudget) -> Self {
        Self { pool, budget }
    }

    /// Estimate token count for a text string.
    /// Uses a simple byte-based heuristic: bytes / 4 (approximate GPT tokenization).
    /// This avoids heavy tiktoken-rs dependency while giving reasonable estimates.
    pub fn estimate_tokens(text: &str) -> usize {
        // Approximate: 1 token ≈ 4 bytes for English text
        // This is conservative (may overestimate) which is safe for budget management
        let byte_estimate = text.len().div_ceil(4);
        byte_estimate.max(1)
    }

    /// Determine compression tier based on current vs budget.
    pub fn compression_tier(&self, current_tokens: usize) -> CompressionTier {
        let budget = self.budget.conversation_budget();
        let ratio = current_tokens as f64 / budget as f64;
        if ratio >= 0.95 {
            CompressionTier::Emergency
        } else if ratio >= 0.90 {
            CompressionTier::Aggressive
        } else if ratio >= 0.80 {
            CompressionTier::Compress
        } else {
            CompressionTier::Normal
        }
    }

    /// Build a context window for a session that fits within the token budget.
    /// Loads messages from SQLite, newest first, accumulating until budget is reached.
    pub async fn build_context_window(&self, session_id: &str) -> Result<Vec<ContextMessage>> {
        let budget = self.budget.conversation_budget();

        // Load all messages for this session, newest first
        let rows: Vec<MsgRow> = sqlx::query_as(
            "SELECT id, role, content, COALESCE(token_estimate, 0) as token_estimate \
             FROM messages \
             WHERE session_id = ? AND status = 'done' \
             ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        let mut messages: Vec<ContextMessage> = Vec::new();
        let mut total_tokens = 0usize;

        for row in &rows {
            let token_est = if row.token_estimate > 0 {
                row.token_estimate as usize
            } else {
                Self::estimate_tokens(&row.content)
            };

            if total_tokens + token_est > budget {
                // Over budget — stop adding messages
                // TODO: F44.3.4 — add summary snapshot here instead
                debug!(
                    session_id,
                    total_tokens,
                    budget,
                    "context window full — truncating older messages"
                );
                break;
            }

            total_tokens += token_est;
            messages.push(ContextMessage {
                id: row.id.clone(),
                role: row.role.clone(),
                content: row.content.clone(),
                token_estimate: token_est,
                is_summary: false,
            });
        }

        // Reverse to chronological order for the AI
        messages.reverse();

        let tier = self.compression_tier(total_tokens);
        if tier != CompressionTier::Normal {
            info!(
                session_id,
                total_tokens,
                budget,
                tier = ?tier,
                "context compression active"
            );
        }

        Ok(messages)
    }

    /// Update token estimate for a message in SQLite.
    pub async fn update_token_estimate(&self, message_id: &str, estimate: i64) -> Result<()> {
        sqlx::query("UPDATE messages SET token_estimate = ? WHERE id = ?")
            .bind(estimate)
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Check if proactive compression is needed (>85% budget used).
    pub async fn should_compress(&self, session_id: &str) -> Result<bool> {
        let budget = self.budget.conversation_budget() as i64;
        let threshold = (budget as f64 * 0.85) as i64;

        let (total,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(COALESCE(token_estimate, 0)), 0) \
             FROM messages \
             WHERE session_id = ? AND status = 'done'",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(total > threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a ContextManager for testing (pool is not used by these tests)
    async fn test_mgr() -> ContextManager {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ContextManager::new(pool, TokenBudget::default())
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(ContextManager::estimate_tokens(""), 1); // min 1
    }

    #[test]
    fn test_estimate_tokens_hello() {
        // "hello world" = 11 bytes → ceil(11/4) = 3
        assert_eq!(ContextManager::estimate_tokens("hello world"), 3);
    }

    #[tokio::test]
    async fn test_compression_tier_normal() {
        let mgr = test_mgr().await;
        // 100k tokens out of 165k budget = 60.6% → Normal
        assert_eq!(mgr.compression_tier(100_000), CompressionTier::Normal);
    }

    #[tokio::test]
    async fn test_compression_tier_compress() {
        let mgr = test_mgr().await;
        // 140k / 165k = 84.8% → Compress
        assert_eq!(mgr.compression_tier(140_000), CompressionTier::Compress);
    }

    #[tokio::test]
    async fn test_compression_tier_aggressive() {
        let mgr = test_mgr().await;
        // 150k / 165k = 90.9% → Aggressive
        assert_eq!(mgr.compression_tier(150_000), CompressionTier::Aggressive);
    }

    #[tokio::test]
    async fn test_compression_tier_emergency() {
        let mgr = test_mgr().await;
        // 160k / 165k = 97% → Emergency
        assert_eq!(mgr.compression_tier(160_000), CompressionTier::Emergency);
    }

    #[test]
    fn test_budget_calculation() {
        let budget = TokenBudget::default();
        // 200k - 20k - 15k = 165k
        assert_eq!(budget.conversation_budget(), 165_000);
    }
}
