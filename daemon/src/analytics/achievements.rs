// SPDX-License-Identifier: MIT
//! Achievement system — 20 pre-defined badges, trigger event type, and unlock logic.
//!
//! Achievement IDs use SCREAMING_SNAKE_CASE as their string value
//! (e.g. `"first_session"`). They are stable across daemon versions.

use anyhow::Result;

use super::model::Achievement;
use super::storage::AnalyticsStorage;

// ─── Achievement ID constants ─────────────────────────────────────────────────

pub const FIRST_SESSION: &str = "first_session";
pub const SESSIONS_10: &str = "sessions_10";
pub const SESSIONS_100: &str = "sessions_100";
pub const SESSIONS_500: &str = "sessions_500";
pub const LINES_1K: &str = "lines_1k";
pub const LINES_10K: &str = "lines_10k";
pub const LINES_100K: &str = "lines_100k";
pub const MULTI_PROVIDER: &str = "multi_provider";
pub const ARENA_VOTER: &str = "arena_voter";
pub const ARENA_CHAMPION: &str = "arena_champion";
pub const PACK_INSTALLER: &str = "pack_installer";
pub const PACK_PUBLISHER: &str = "pack_publisher";
pub const DOCTOR_A_PLUS: &str = "doctor_a_plus";
pub const WORKFLOW_AUTOMATOR: &str = "workflow_automator";
pub const DRIFT_FIXER: &str = "drift_fixer";
pub const GIT_POWER_USER: &str = "git_power_user";
pub const NIGHT_OWL: &str = "night_owl";
pub const SPEED_CODER: &str = "speed_coder";
pub const MOBILE_USER: &str = "mobile_user";
pub const ACHIEVEMENT_HUNTER: &str = "achievement_hunter";

// ─── Achievement definitions ──────────────────────────────────────────────────

/// Return all defined achievements as `(id, name, description)` tuples.
///
/// This list is the canonical source of truth for the achievement catalogue.
/// The Flutter `achievements_screen.dart` reads this via `achievements.list` RPC
/// and renders each badge (earned = full color; unearned = grayscale).
pub fn all_definitions() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            FIRST_SESSION,
            "First Session",
            "Started your first AI session. The journey begins.",
        ),
        (
            SESSIONS_10,
            "Getting Warmed Up",
            "Completed 10 AI sessions.",
        ),
        (
            SESSIONS_100,
            "Century",
            "Completed 100 AI sessions.",
        ),
        (
            SESSIONS_500,
            "Power User",
            "Completed 500 AI sessions. You are ClawDE.",
        ),
        (
            LINES_1K,
            "1K Lines",
            "Wrote 1,000 lines of code with AI assistance.",
        ),
        (
            LINES_10K,
            "10K Lines",
            "Wrote 10,000 lines of code with AI assistance.",
        ),
        (
            LINES_100K,
            "100K Lines",
            "Wrote 100,000 lines of code with AI assistance. Prolific.",
        ),
        (
            MULTI_PROVIDER,
            "Multi-Provider",
            "Used more than one AI provider in the same project.",
        ),
        (
            ARENA_VOTER,
            "Arena Voter",
            "Cast your first Arena vote — provider A or B?",
        ),
        (
            ARENA_CHAMPION,
            "Arena Champion",
            "Cast 50 Arena votes. Your leaderboard shapes the routing.",
        ),
        (
            PACK_INSTALLER,
            "Pack Installer",
            "Installed your first ClawDE pack.",
        ),
        (
            PACK_PUBLISHER,
            "Pack Publisher",
            "Published a pack to the ClawDE registry.",
        ),
        (
            DOCTOR_A_PLUS,
            "Doctor A+",
            "Achieved a perfect score on `clawd doctor`.",
        ),
        (
            WORKFLOW_AUTOMATOR,
            "Workflow Automator",
            "Created your first automated workflow.",
        ),
        (
            DRIFT_FIXER,
            "Drift Fixer",
            "Resolved at least one drift warning with AI assistance.",
        ),
        (
            GIT_POWER_USER,
            "Git Power User",
            "Ran git operations via ClawDE in 10 different sessions.",
        ),
        (
            NIGHT_OWL,
            "Night Owl",
            "Completed a session after midnight, local time.",
        ),
        (
            SPEED_CODER,
            "Speed Coder",
            "Sent 10 messages in under 5 minutes in a single session.",
        ),
        (
            MOBILE_USER,
            "Mobile User",
            "Connected to your daemon from the ClawDE mobile app.",
        ),
        (
            ACHIEVEMENT_HUNTER,
            "Achievement Hunter",
            "Unlocked 10 or more achievements.",
        ),
    ]
}

// ─── Trigger event type ───────────────────────────────────────────────────────

/// Events that trigger achievement checks.
///
/// Each variant carries only the data needed to evaluate the relevant
/// achievement conditions. Handlers emit one of these after completing
/// the relevant action (e.g. after `session.create` succeeds, emit
/// `AchievementEvent::SessionCreated { total_sessions: N }`).
#[derive(Debug, Clone)]
pub enum AchievementEvent {
    /// A new session was created. `total_sessions` is the cumulative count.
    SessionCreated { total_sessions: u64 },

    /// A git operation was used in a session.
    GitOpUsed { git_sessions: u64 },

    /// An Arena vote was cast. `total_votes` is the cumulative count.
    ArenaVoted { total_votes: u64 },

    /// A pack was installed for the first time.
    PackInstalled,

    /// A pack was published to the registry.
    PackPublished,

    /// `clawd doctor` returned a perfect score.
    DoctorAPlus,

    /// A drift warning was resolved via AI.
    DriftResolved,

    /// The mobile app connected to this daemon.
    MobileConnected,

    /// A workflow was created.
    WorkflowCreated,

    /// A session used a second distinct provider in the same project.
    MultiProviderUsed,

    /// Lines written total crossed a threshold.
    LinesWritten { total_lines: u64 },

    /// 10+ achievements already unlocked (checked after any unlock).
    CheckAchievementHunter { total_unlocked: u64 },

    /// A session was conducted after midnight local time.
    NightOwlSession,

    /// 10 messages sent in < 5 minutes in one session.
    SpeedCoderSession,
}

// ─── Trigger check ────────────────────────────────────────────────────────────

/// Evaluate `event` against achievement conditions, unlock any newly met
/// achievements, and return the list of newly unlocked achievements so the
/// caller can broadcast `achievement.unlocked` push events.
pub async fn check_and_unlock(
    storage: &AnalyticsStorage,
    event: &AchievementEvent,
) -> Result<Vec<Achievement>> {
    let mut newly_unlocked = Vec::new();

    let candidates: Vec<&str> = match event {
        AchievementEvent::SessionCreated { total_sessions } => {
            let mut ids = vec![];
            if *total_sessions >= 1 {
                ids.push(FIRST_SESSION);
            }
            if *total_sessions >= 10 {
                ids.push(SESSIONS_10);
            }
            if *total_sessions >= 100 {
                ids.push(SESSIONS_100);
            }
            if *total_sessions >= 500 {
                ids.push(SESSIONS_500);
            }
            ids
        }
        AchievementEvent::LinesWritten { total_lines } => {
            let mut ids = vec![];
            if *total_lines >= 1_000 {
                ids.push(LINES_1K);
            }
            if *total_lines >= 10_000 {
                ids.push(LINES_10K);
            }
            if *total_lines >= 100_000 {
                ids.push(LINES_100K);
            }
            ids
        }
        AchievementEvent::ArenaVoted { total_votes } => {
            let mut ids = vec![];
            if *total_votes >= 1 {
                ids.push(ARENA_VOTER);
            }
            if *total_votes >= 50 {
                ids.push(ARENA_CHAMPION);
            }
            ids
        }
        AchievementEvent::GitOpUsed { git_sessions } => {
            let mut ids = vec![];
            if *git_sessions >= 10 {
                ids.push(GIT_POWER_USER);
            }
            ids
        }
        AchievementEvent::PackInstalled => vec![PACK_INSTALLER],
        AchievementEvent::PackPublished => vec![PACK_PUBLISHER],
        AchievementEvent::DoctorAPlus => vec![DOCTOR_A_PLUS],
        AchievementEvent::DriftResolved => vec![DRIFT_FIXER],
        AchievementEvent::MobileConnected => vec![MOBILE_USER],
        AchievementEvent::WorkflowCreated => vec![WORKFLOW_AUTOMATOR],
        AchievementEvent::MultiProviderUsed => vec![MULTI_PROVIDER],
        AchievementEvent::CheckAchievementHunter { total_unlocked } => {
            if *total_unlocked >= 10 {
                vec![ACHIEVEMENT_HUNTER]
            } else {
                vec![]
            }
        }
        AchievementEvent::NightOwlSession => vec![NIGHT_OWL],
        AchievementEvent::SpeedCoderSession => vec![SPEED_CODER],
    };

    // Build a quick lookup of (id -> (name, description)) from definitions.
    let defs: std::collections::HashMap<&str, (&str, &str)> = all_definitions()
        .into_iter()
        .map(|(id, name, desc)| (id, (name, desc)))
        .collect();

    for id in candidates {
        let is_new = storage.unlock_achievement(id).await?;
        if is_new {
            let (name, description) = defs.get(id).copied().unwrap_or(("Unknown", ""));
            let unlocked_at = chrono::Utc::now().to_rfc3339();
            newly_unlocked.push(Achievement {
                id: id.to_string(),
                name: name.to_string(),
                description: description.to_string(),
                unlocked: true,
                unlocked_at: Some(unlocked_at),
            });
        }
    }

    // If any were newly unlocked, check whether ACHIEVEMENT_HUNTER is now due.
    if !newly_unlocked.is_empty() {
        let all = storage.list_achievements().await?;
        let total_unlocked = all.iter().filter(|a| a.unlocked).count() as u64;
        if total_unlocked >= 10 {
            let is_new = storage.unlock_achievement(ACHIEVEMENT_HUNTER).await?;
            if is_new {
                let (name, description) = defs
                    .get(ACHIEVEMENT_HUNTER)
                    .copied()
                    .unwrap_or(("Achievement Hunter", ""));
                let unlocked_at = chrono::Utc::now().to_rfc3339();
                newly_unlocked.push(Achievement {
                    id: ACHIEVEMENT_HUNTER.to_string(),
                    name: name.to_string(),
                    description: description.to_string(),
                    unlocked: true,
                    unlocked_at: Some(unlocked_at),
                });
            }
        }
    }

    Ok(newly_unlocked)
}
