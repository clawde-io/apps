// metrics/budget.rs — Rolling budget enforcement (Sprint PP OB.3).
//
// Checks daily/monthly cost totals against configured limits and returns
// BudgetStatus. The session runner calls this after each metric tick and
// emits `budget_warning` / `budget_exceeded` push events accordingly.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BudgetStatus {
    Ok,
    Warning { pct: u8 },  // ≥80% of limit
    Exceeded { pct: u8 }, // ≥100% of limit
}

impl BudgetStatus {
    pub fn is_exceeded(&self) -> bool {
        matches!(self, Self::Exceeded { .. })
    }

    pub fn as_push_event(&self) -> Option<(&'static str, serde_json::Value)> {
        match self {
            Self::Warning { pct } => Some((
                "budget_warning",
                serde_json::json!({ "threshold_pct": pct }),
            )),
            Self::Exceeded { pct } => Some((
                "budget_exceeded",
                serde_json::json!({ "threshold_pct": pct }),
            )),
            Self::Ok => None,
        }
    }
}

/// Evaluate current spend against limits.
///
/// Returns the most severe status (exceeded > warning > ok).
pub fn evaluate_budget(
    daily_cost: f64,
    monthly_cost: f64,
    daily_limit: Option<f64>,
    monthly_limit: Option<f64>,
) -> BudgetStatus {
    let daily_status = check_limit(daily_cost, daily_limit);
    let monthly_status = check_limit(monthly_cost, monthly_limit);

    // Return the more severe of the two
    match (&daily_status, &monthly_status) {
        (BudgetStatus::Exceeded { pct }, _) => BudgetStatus::Exceeded { pct: *pct },
        (_, BudgetStatus::Exceeded { pct }) => BudgetStatus::Exceeded { pct: *pct },
        (BudgetStatus::Warning { pct }, _) => BudgetStatus::Warning { pct: *pct },
        (_, BudgetStatus::Warning { pct }) => BudgetStatus::Warning { pct: *pct },
        _ => BudgetStatus::Ok,
    }
}

fn check_limit(cost: f64, limit: Option<f64>) -> BudgetStatus {
    let Some(limit) = limit else {
        return BudgetStatus::Ok;
    };
    if limit <= 0.0 {
        return BudgetStatus::Ok;
    }
    let pct = ((cost / limit) * 100.0).round() as u8;
    if pct >= 100 {
        BudgetStatus::Exceeded { pct }
    } else if pct >= 80 {
        BudgetStatus::Warning { pct }
    } else {
        BudgetStatus::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_when_no_limits() {
        assert_eq!(evaluate_budget(100.0, 500.0, None, None), BudgetStatus::Ok);
    }

    #[test]
    fn test_daily_warning_at_80pct() {
        let status = evaluate_budget(8.0, 0.0, Some(10.0), None);
        assert!(matches!(status, BudgetStatus::Warning { .. }));
    }

    #[test]
    fn test_exceeded_at_100pct() {
        let status = evaluate_budget(10.5, 0.0, Some(10.0), None);
        assert!(status.is_exceeded());
    }

    #[test]
    fn test_monthly_wins_over_daily_ok() {
        let status = evaluate_budget(1.0, 95.0, Some(10.0), Some(100.0));
        assert!(matches!(status, BudgetStatus::Warning { .. }));
    }

    #[test]
    fn test_push_event_format() {
        let status = BudgetStatus::Warning { pct: 85 };
        let (event, payload) = status.as_push_event().unwrap();
        assert_eq!(event, "budget_warning");
        assert_eq!(payload["threshold_pct"], 85);
    }
}
