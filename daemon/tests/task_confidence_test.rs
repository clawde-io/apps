//! Sprint CC TC.5 — Task confidence score tests.

// ─── Confidence score parsing ──────────────────────────────────────────────

/// Parse confidence score from AI message text.
/// Looks for patterns like "0.85", "Confidence: 0.9", "confidence score: 0.7".
fn parse_confidence(text: &str) -> Option<(f64, String)> {
    // Look for lines starting with "confidence" (case-insensitive).
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("confidence") {
            // Find a float in the line.
            for part in line.split_whitespace() {
                let clean = part.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if let Ok(score) = clean.parse::<f64>() {
                    if (0.0..=1.0).contains(&score) {
                        // Extract the rest as reasoning.
                        let reasoning = line.trim().to_string();
                        return Some((score, reasoning));
                    }
                }
            }
        }
    }
    None
}

/// Confidence badge color based on score.
fn badge_color(score: f64) -> &'static str {
    if score >= 0.8 {
        "green"
    } else if score >= 0.5 {
        "amber"
    } else {
        "red"
    }
}

#[test]
fn parse_confidence_from_ai_message() {
    let msg = "I've completed the refactoring task.\nConfidence: 0.85 — all tests pass and no regressions found.";
    let result = parse_confidence(msg);
    assert!(result.is_some());
    let (score, _) = result.unwrap();
    assert!((score - 0.85).abs() < 0.001);
}

#[test]
fn parse_confidence_low_value() {
    let msg = "I attempted the task but could not verify all edge cases.\nconfidence score: 0.4 — some paths untested.";
    let result = parse_confidence(msg);
    assert!(result.is_some());
    let (score, _) = result.unwrap();
    assert!((score - 0.4).abs() < 0.001);
}

#[test]
fn parse_confidence_returns_none_when_absent() {
    let msg = "Task completed successfully. All tests pass.";
    let result = parse_confidence(msg);
    assert!(result.is_none(), "should not parse confidence when not present");
}

#[test]
fn badge_color_green_for_high_confidence() {
    assert_eq!(badge_color(0.9), "green");
    assert_eq!(badge_color(0.8), "green");
}

#[test]
fn badge_color_amber_for_medium_confidence() {
    assert_eq!(badge_color(0.79), "amber");
    assert_eq!(badge_color(0.5), "amber");
}

#[test]
fn badge_color_red_for_low_confidence() {
    assert_eq!(badge_color(0.49), "red");
    assert_eq!(badge_color(0.0), "red");
}

#[test]
fn badge_color_boundary_values() {
    assert_eq!(badge_color(1.0), "green");
    assert_eq!(badge_color(0.8), "green");
    assert_eq!(badge_color(0.5), "amber");
}
