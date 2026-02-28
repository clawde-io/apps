-- Sprint M: Pack Marketplace (PK.T03)
-- Tracks installed packs: name, version, type, install path, optional signature.
CREATE TABLE IF NOT EXISTS installed_packs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    version     TEXT NOT NULL,
    pack_type   TEXT NOT NULL,
    publisher   TEXT,
    description TEXT,
    install_path TEXT NOT NULL,
    signature   TEXT,
    installed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(name)
);
