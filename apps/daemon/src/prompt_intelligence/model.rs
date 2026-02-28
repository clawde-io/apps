// SPDX-License-Identifier: MIT
//! Data types for the prompt intelligence subsystem.

use serde::{Deserialize, Serialize};

/// Where a suggestion originated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionSource {
    /// Derived from the user's own prompt history.
    History,
    /// A built-in template matched by keyword prefix.
    Template,
    /// Generated from the current session / file context.
    Context,
    /// Matched from a saved workflow definition.
    Workflow,
}

/// A single prompt suggestion returned to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSuggestion {
    /// Stable identifier for this suggestion (used for dedup and tracking).
    pub id: String,
    /// The full suggested prompt text.
    pub text: String,
    /// Human-readable label for why this was suggested.
    pub context: String,
    /// 0.0–1.0 relevance score (higher = show first).
    pub score: f32,
    /// How this suggestion was produced.
    pub source: SuggestionSource,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestion_source_serde_roundtrip() {
        let cases = [
            SuggestionSource::History,
            SuggestionSource::Template,
            SuggestionSource::Context,
            SuggestionSource::Workflow,
        ];
        for source in cases {
            let json = serde_json::to_string(&source).unwrap();
            let back: SuggestionSource = serde_json::from_str(&json).unwrap();
            assert_eq!(back, source);
        }
    }

    #[test]
    fn suggestion_source_camel_case_serialisation() {
        // serde(rename_all = "camelCase") means History → "history" etc.
        let json = serde_json::to_string(&SuggestionSource::History).unwrap();
        assert_eq!(json, "\"history\"");
        let json2 = serde_json::to_string(&SuggestionSource::Template).unwrap();
        assert_eq!(json2, "\"template\"");
    }

    #[test]
    fn prompt_suggestion_fields_accessible() {
        let s = PromptSuggestion {
            id: "tpl:abc".to_string(),
            text: "Fix the error in the current file.".to_string(),
            context: "Error fix template".to_string(),
            score: 0.85,
            source: SuggestionSource::Template,
        };
        assert_eq!(s.source, SuggestionSource::Template);
        assert!((s.score - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn prompt_suggestion_roundtrip_json() {
        let s = PromptSuggestion {
            id: "hist:xyz".to_string(),
            text: "Refactor this module.".to_string(),
            context: "Used 3 times".to_string(),
            score: 0.75,
            source: SuggestionSource::History,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: PromptSuggestion = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "hist:xyz");
        assert_eq!(back.source, SuggestionSource::History);
    }

    #[test]
    fn prompt_suggestion_score_range() {
        // Scores should be in [0.0, 1.0] — verify a boundary value survives
        let s = PromptSuggestion {
            id: "ctx:0".to_string(),
            text: "hello".to_string(),
            context: "ctx".to_string(),
            score: 0.0,
            source: SuggestionSource::Context,
        };
        assert!((s.score - 0.0).abs() < f32::EPSILON);
        let s2 = PromptSuggestion {
            id: "ctx:1".to_string(),
            text: "hi".to_string(),
            context: "ctx".to_string(),
            score: 1.0,
            source: SuggestionSource::Context,
        };
        assert!((s2.score - 1.0).abs() < f32::EPSILON);
    }
}
