-- SPDX-License-Identifier: MIT
-- Sprint N: Multi-Repo Orchestration (MR.T01, MR.T06)
--
-- repo_dependencies: tracks directed dependency edges between registered repos.
-- mailbox_messages:  persists cross-repo inbox messages managed by the daemon.

CREATE TABLE IF NOT EXISTS repo_dependencies (
    id          TEXT    PRIMARY KEY,
    from_repo   TEXT    NOT NULL,
    to_repo     TEXT    NOT NULL,
    dep_type    TEXT    NOT NULL DEFAULT 'uses_api',
    -- Heuristic confidence 0.0–1.0; manually-declared edges always use 1.0.
    confidence  REAL    NOT NULL DEFAULT 1.0,
    -- 1 = discovered by the auto-detector; 0 = manually declared by the user.
    auto_detected INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_repo, to_repo)
);

CREATE TABLE IF NOT EXISTS mailbox_messages (
    id          TEXT    PRIMARY KEY,
    from_repo   TEXT    NOT NULL,
    to_repo     TEXT    NOT NULL,
    subject     TEXT    NOT NULL,
    body        TEXT    NOT NULL,
    reply_to    TEXT,
    expires_at  TEXT,
    -- 1 = archived (processed); 0 = unread / pending.
    archived    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
