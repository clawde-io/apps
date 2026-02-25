// SPDX-License-Identifier: MIT
//! IDE extension host module — Sprint Z, IE.T01–IE.T08.
//!
//! Provides bi-directional integration between `clawd` and editor extensions
//! (VS Code, JetBrains, Neovim, Emacs).  Extensions connect over the same
//! JSON-RPC 2.0 WebSocket as Flutter clients and call the `ide.*` namespace.
//!
//! ## Sub-modules
//!
//! - [`editor_context`] — `EditorContext` and `IdeConnectionRecord` types
//! - [`vscode_bridge`]  — in-memory registry of connected extensions + their contexts
//! - [`handlers`]       — RPC handlers wired into the `ipc/mod.rs` dispatch table
//!
//! ## RPC surface
//!
//! | Method                   | Caller | Description                                    |
//! |--------------------------|--------|------------------------------------------------|
//! | `ide.extensionConnected` | IDE    | Register a new extension connection            |
//! | `ide.editorContext`      | IDE    | Push current editor state (file, cursor, …)    |
//! | `ide.syncSettings`       | App    | Broadcast settings to all connected extensions |
//! | `ide.listConnections`    | App    | List all connected extensions                  |
//! | `ide.latestContext`      | App    | Get most-recent editor context                 |
//!
//! ## Push events emitted
//!
//! | Event                   | Trigger                         |
//! |-------------------------|---------------------------------|
//! | `ide.extensionConnected`| A new extension connects        |
//! | `editor.contextChanged` | Extension pushes a new context  |
//! | `settings.changed`      | App calls `ide.syncSettings`    |
//!
//! ## Wiring
//!
//! See `sprint_Z_wiring_notes.md` for the exact lines to add to
//! `apps/daemon/src/lib.rs` and `apps/daemon/src/ipc/mod.rs`.

pub mod editor_context;
pub mod handlers;
pub mod vscode_bridge;

pub use vscode_bridge::{new_shared_bridge, SharedVsCodeBridge};

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Return the current UTC time as an ISO-8601 string (`2006-01-02T15:04:05Z`).
///
/// Used by [`editor_context::EditorContext::new`] and [`vscode_bridge`] for
/// connection timestamps — avoids pulling in a full `chrono` dependency just
/// for this module by using the standard library.
pub(crate) fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Fast manual ISO-8601 formatting from a Unix timestamp.
    // Accurate for dates in the range 1970–2099 (sufficient for operational use).
    let (y, mo, d, h, mi, s) = unix_to_parts(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

/// Decompose a Unix timestamp (seconds) into `(year, month, day, hour, min, sec)`.
fn unix_to_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let mi = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Julian-day-number method for Gregorian calendar.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month of year (March=0)
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };

    (y, mo, d, h, mi, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_1970() {
        let (y, mo, d, h, mi, s) = unix_to_parts(0);
        assert_eq!((y, mo, d, h, mi, s), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn known_timestamp() {
        // 2024-01-15T11:50:45Z = 1705319445 (verified via python datetime.utcfromtimestamp)
        let (y, mo, d, h, mi, s) = unix_to_parts(1_705_319_445);
        assert_eq!((y, mo, d), (2024, 1, 15));
        assert_eq!((h, mi, s), (11, 50, 45));
    }

    #[test]
    fn now_utc_format() {
        let ts = now_utc();
        // Should be exactly 20 chars: "YYYY-MM-DDTHH:MM:SSZ"
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
    }
}
