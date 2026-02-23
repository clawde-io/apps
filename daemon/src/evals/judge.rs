//! LLM-as-judge scoring stub.
//!
//! In production this would call an LLM API with a structured rubric to score
//! agent output quality on a 0–10 scale.  The stub returns a fixed neutral score
//! until an API integration is configured.

// ─── Types ────────────────────────────────────────────────────────────────────

/// Score produced by the LLM judge for a piece of agent output.
#[derive(Debug)]
pub struct JudgeScore {
    /// Score on a 0–10 scale (10 = perfect).
    pub score: u8,
    /// Reasoning provided by the judge.
    pub reasoning: String,
}

// ─── Scoring ─────────────────────────────────────────────────────────────────

/// Score agent output against a rubric using an LLM judge.
///
/// Currently a stub — returns `score: 5` with a note that the judge is not
/// yet configured.  Replace the body with an actual LLM API call when ready.
pub async fn score_output(_output: &str, _rubric: &str) -> anyhow::Result<JudgeScore> {
    Ok(JudgeScore {
        score: 5,
        reasoning: "LLM judge not yet configured — returning neutral score.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_returns_neutral_score() {
        let result = score_output("some output", "quality rubric").await.unwrap();
        assert_eq!(result.score, 5);
        assert!(!result.reasoning.is_empty());
    }
}
