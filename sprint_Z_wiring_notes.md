# Sprint Z — Wiring Notes

This file documents exactly what must be added to existing daemon files to wire up
the Sprint Z modules created in this sprint.

**Do NOT modify the files below during this sprint** — these are wiring-only changes
that should be applied in a separate commit after all Sprint Z files are reviewed.

---

## 1. `apps/daemon/src/lib.rs` — Add module declarations

### Add after the `pub mod vscode;` line (around line 164)

```rust
// Sprint Z — IDE Extension Host
pub mod ide;

// Sprint Z — Performance & Scale
pub mod perf;
```

### Add to `AppContext` struct (after the `metrics: SharedMetrics` field, around line 105)

```rust
/// In-memory registry of connected IDE extensions and their editor contexts (Sprint Z).
pub ide_bridge: crate::ide::SharedVsCodeBridge,
```

### Add to `AppContext` initialisation in `main.rs`

When constructing the `AppContext`, add:

```rust
ide_bridge: crate::ide::new_shared_bridge(),
```

### Add to `init_scheduler_and_worktrees` if using a builder pattern

If `AppContext` is constructed via a builder/default, also initialise:

```rust
ide_bridge: crate::ide::new_shared_bridge(),
```

---

## 2. `apps/daemon/src/ipc/mod.rs` — Add dispatch entries

### In the `dispatch` function, add after the Sprint S LSP block (around line 800)

```rust
// ─── Sprint Z: IDE Extension Integration ─────────────────────────────────────
"ide.extensionConnected" => crate::ide::handlers::extension_connected(params, ctx).await,
"ide.editorContext"      => crate::ide::handlers::editor_context(params, ctx).await,
"ide.syncSettings"       => crate::ide::handlers::sync_settings(params, ctx).await,
"ide.listConnections"    => crate::ide::handlers::list_connections(params, ctx).await,
"ide.latestContext"      => crate::ide::handlers::latest_context(params, ctx).await,
```

These must be placed before the `_ => Err(...)` catch-all at the end of the match.

---

## 3. `apps/daemon/src/storage/mod.rs` — Register new migration

The `038_enterprise_policies.sql` migration is already in the migrations directory.
SQLx's `migrate!()` macro picks up files alphabetically — no code change needed
IF the macro is already scanning the full migrations directory.

**Verify** by checking the `migrate!` macro invocation in `storage/mod.rs`:

```rust
sqlx::migrate!("src/storage/migrations")
    .run(pool)
    .await?;
```

If this pattern is used, the migration runs automatically. If migrations are
listed explicitly (uncommon), add:

```rust
include_str!("migrations/038_enterprise_policies.sql"),
```

---

## 4. Cargo.toml additions needed

The new modules use only crates already in `Cargo.toml`:
- `tokio` (async, sync, time) — already present
- `anyhow` — already present
- `serde` / `serde_json` — already present
- `tracing` — already present
- `uuid` — already present
- `sqlx` — already present

**No new dependencies required** for the `ide/` and `perf/wal_tuning.rs` modules.

The `perf/connection_pool.rs` module has a `// TODO(Z.3)` stub for
`tokio_tungstenite::connect_async`. When the stub is replaced with a real
WebSocket connection, `tokio-tungstenite` must already be in Cargo.toml (it is,
under the `relay` feature flag). Confirm with:

```sh
grep -E "tokio.tungstenite|tungstenite" apps/daemon/Cargo.toml
```

---

## 5. `apps/daemon/src/ipc/handlers/mod.rs` — No changes needed

The `ide::handlers` functions are called directly via the `crate::ide::handlers::`
path from `ipc/mod.rs`, matching the pattern used by Sprint P (`crate::builder::handlers`)
and Sprint J (`crate::autonomous::handlers`). No entry in `handlers/mod.rs` needed.

---

## 6. Enterprise TypeScript modules — No wiring needed immediately

The files in `web/backend/src/modules/enterprise/` are pure TypeScript modules.
They export functions and types but do not register routes or start any services.

To wire them into the Express API:
1. Create `web/backend/services/clawde-api/routes/enterprise.js` (or `.ts` once
   the backend migrates to TypeScript)
2. Import the enterprise functions
3. Register the router in `index.js`

Route surface to expose:
- `GET  /api/enterprise/policies?org_id=...`
- `POST /api/enterprise/policies`
- `PATCH /api/enterprise/policies/:id`
- `DELETE /api/enterprise/policies/:id`
- `GET  /api/enterprise/audit-log?org_id=...&from=...&to=...&format=json|csv`
- `GET  /api/enterprise/sso?org_id=...`
- `POST /api/enterprise/sso`
- `DELETE /api/enterprise/sso?org_id=...`

---

## 7. Web app — Enterprise route

Add `/enterprise` to the router in `web/app/src/App.tsx`:

```tsx
import { EnterprisePage } from "./pages/enterprise";

// In the Routes block:
<Route path="/enterprise" element={<EnterprisePage />} />
```

---

## 8. Performance module — startup wiring

To apply WAL tuning at daemon startup, add to `main.rs` after the storage pool is
created and migrations have run:

```rust
crate::perf::wal_tuning::apply_wal_tuning(storage.pool())
    .await
    .expect("SQLite WAL tuning failed");
```

To checkpoint on clean shutdown, add to the graceful shutdown handler:

```rust
if let Err(e) = crate::perf::wal_tuning::checkpoint_wal(
    storage.pool(),
    "TRUNCATE",
).await {
    tracing::warn!(err = %e, "WAL checkpoint on shutdown failed");
}
```

---

## Summary of new files created in Sprint Z

### Rust (apps/)

| File | Module | Purpose |
| --- | --- | --- |
| `daemon/src/ide/mod.rs` | `ide` | IDE extension host — module root |
| `daemon/src/ide/editor_context.rs` | `ide::editor_context` | EditorContext + IdeConnectionRecord types |
| `daemon/src/ide/vscode_bridge.rs` | `ide::vscode_bridge` | In-memory extension registry |
| `daemon/src/ide/handlers.rs` | `ide::handlers` | 5 RPC handlers |
| `daemon/src/perf/mod.rs` | `perf` | Performance module root |
| `daemon/src/perf/wal_tuning.rs` | `perf::wal_tuning` | SQLite WAL PRAGMAs |
| `daemon/src/perf/connection_pool.rs` | `perf::connection_pool` | WebSocket connection pool |
| `daemon/src/storage/migrations/038_enterprise_policies.sql` | — | DB migration |

### TypeScript (web/)

| File | Purpose |
| --- | --- |
| `backend/src/modules/enterprise/types.ts` | Enterprise type definitions |
| `backend/src/modules/enterprise/policies.ts` | Policy engine + Hasura queries |
| `backend/src/modules/enterprise/audit.ts` | Audit log export helpers |
| `backend/src/modules/enterprise/sso.ts` | SSO config + validation |
| `app/src/pages/enterprise/index.tsx` | Enterprise admin portal page |

### Wiki (apps/.wiki/)

| File | Purpose |
| --- | --- |
| `.wiki/RPC-Reference.md` | Complete 174-method RPC reference |
| `.wiki/CLI-Reference.md` | All clawd CLI commands |
| `.wiki/Enterprise.md` | Enterprise deployment guide |
