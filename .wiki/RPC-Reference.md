# RPC Reference

Complete reference for all 174+ JSON-RPC 2.0 methods exposed by `clawd` on port 4300.

All methods follow the JSON-RPC 2.0 specification over WebSocket. Before calling any method a client must authenticate with `daemon.auth`. Push events are broadcast to all connected clients; they are not responses to a request.

## Authentication

Every connection must send `daemon.auth` as its first message:

```json
{ "jsonrpc": "2.0", "id": 1, "method": "daemon.auth", "params": { "token": "<auth_token>" } }
```

The token is stored in `{data_dir}/auth_token` (mode 0600). On success the daemon responds `{ "authenticated": true }`. All subsequent messages on the same connection are accepted.

---

## Error codes

| Code | Constant | Meaning |
| --- | --- | --- |
| -32700 | `PARSE_ERROR` | Malformed JSON |
| -32600 | `INVALID_REQUEST` | Not a valid JSON-RPC 2.0 request |
| -32601 | `METHOD_NOT_FOUND` | Method does not exist |
| -32602 | `INVALID_PARAMS` | Missing or wrong-type params |
| -32603 | `INTERNAL_ERROR` | Unexpected server error |
| -32001 | `sessionNotFound` | Session ID does not exist |
| -32002 | `providerNotAvailable` | Session busy — a turn is running |
| -32003 | `rateLimited` | AI provider rate limit hit |
| -32004 | `unauthorized` | Bad or missing auth token |
| -32005 | `repoNotFound` | Not a git repo or path missing |
| -32006 | `sessionPaused` | Session paused — call `session.resume` first |
| -32007 | `sessionLimitReached` | Max session count reached |
| -32010 | `taskNotFound` | Task ID does not exist |
| -32011 | `taskAlreadyClaimed` | Task claimed by another agent |
| -32014 | `missingCompletionNotes` | Completion notes required when marking done |
| -32016 | `modeViolation` | Tool rejected — session is in FORGE or STORM mode |
| -32028 | `toolSecurityBlocked` | Tool call blocked by security policy |
| -32029 | `ipcRateLimited` | Per-connection RPC rate limit exceeded |

---

## Namespaces

| Namespace | Methods | Description |
| --- | --- | --- |
| `account.*` | 5 | Multi-account management |
| `ae.*` | 7 | Autonomous execution engine |
| `afs.*` | 4 | AFS (AI Filesystem) management |
| `agents.*` | 4 | Multi-agent orchestration |
| `analytics.*` | 4 | Personal + provider analytics |
| `approval.*` | 2 | Human approval workflow |
| `arena.*` | 3 | Arena mode (multi-model comparison) |
| `browser.*` | 1 | Browser tool (screenshot) |
| `builder.*` | 3 | Builder mode |
| `completion.*` | 1 | Code completion suggestions |
| `context.*` | 1 | Context bridging |
| `daemon.*` | 10 | Daemon lifecycle + info |
| `device.*` | 4 | Device pairing |
| `doctor.*` | 4 | Daemon diagnostics |
| `drift.*` | 2 | Drift scanner |
| `ide.*` | 5 | IDE extension integration (Sprint Z) |
| `license.*` | 3 | License tier gating |
| `lsp.*` | 5 | Language Server Protocol proxy |
| `mailbox.*` | 3 | Multi-repo cross-daemon messaging |
| `message.*` | 2 | Message pin/unpin |
| `onboarding.*` | 9 | Provider onboarding |
| `packs.*` | 5 | Pack marketplace |
| `project.*` | 7 | Project management |
| `prompt.*` | 2 | Prompt intelligence |
| `providers.*` | 2 | Provider detection |
| `repo.*` | 11 | Git repo management |
| `review.*` | 3 | AI code review |
| `scheduler.*` | 1 | Account scheduler status |
| `session.*` | 16 | AI session lifecycle |
| `standards.*` | 1 | Coding standards |
| `system.*` | 2 | System resource monitoring |
| `tasks.*` | 20 | Task system |
| `tasks.agents.*` | 4 | Task agent registry |
| `te.*` | 14 | Task engine (Phase 45) |
| `threads.*` | 4 | Conversation threading |
| `token.*` | 3 | Token usage tracking |
| `tool.*` | 2 | Tool call approve/reject |
| `topology.*` | 5 | Multi-repo dependency topology |
| `traces.*` | 2 | Observability traces |
| `validators.*` | 2 | Repo validators |
| `worktrees.*` | 9 | Per-task Git worktrees |

---

## account.*

### account.list
List all configured AI accounts.

**Params:** none
**Returns:** `{ accounts: Account[] }`

### account.create
Add a new AI account.

**Params:** `{ provider: string, api_key: string, label?: string }`
**Returns:** `{ account_id: string }`

### account.delete
Delete an AI account by ID.

**Params:** `{ account_id: string }`
**Returns:** `{ deleted: true }`

### account.setPriority
Set the priority order of an account in the rotation.

**Params:** `{ account_id: string, priority: number }`
**Returns:** `{ updated: true }`

### account.history
Return usage history for an account.

**Params:** `{ account_id: string, limit?: number }`
**Returns:** `{ events: AccountEvent[] }`

---

## ae.* (Autonomous Execution Engine)

### ae.plan.create
Create an autonomous execution plan for a set of tasks.

**Params:** `{ task_ids: string[], repo_path: string, goal?: string }`
**Returns:** `{ plan_id: string, phases: PlanPhase[] }`

### ae.plan.approve
Approve an autonomous plan, allowing it to execute.

**Params:** `{ plan_id: string }`
**Returns:** `{ approved: true }`

### ae.plan.get
Get the status and phases of a plan.

**Params:** `{ plan_id: string }`
**Returns:** `AePlan`

### ae.decision.record
Record a decision made during autonomous execution.

**Params:** `{ plan_id: string, decision: string, rationale: string }`
**Returns:** `{ decision_id: string }`

### ae.confidence.get
Return the current confidence score for a plan's execution.

**Params:** `{ plan_id: string }`
**Returns:** `{ score: number, factors: ConfidenceFactor[] }`

### ae.recipe.list
List all saved automation recipes.

**Params:** `{ limit?: number }`
**Returns:** `{ recipes: AeRecipe[] }`

### ae.recipe.create
Create a reusable automation recipe.

**Params:** `{ name: string, description: string, steps: RecipeStep[] }`
**Returns:** `{ recipe_id: string }`

---

## afs.*

### afs.init
Initialise AFS tracking for a workspace.

**Params:** `{ repo_path: string }`
**Returns:** `{ initialised: true }`

### afs.status
Return AFS status for a workspace.

**Params:** `{ repo_path: string }`
**Returns:** `AfsStatus`

### afs.syncInstructions
Sync `.claude/CLAUDE.md` and related instruction files.

**Params:** `{ repo_path: string }`
**Returns:** `{ synced: number }`

### afs.register
Register a project with AFS.

**Params:** `{ repo_path: string, project_id?: string }`
**Returns:** `{ registered: true }`

---

## agents.*

### agents.spawn
Spawn a new orchestrated agent for a task.

**Params:** `{ task_id: string, role: AgentRole, provider?: string }`
**Returns:** `{ agent_id: string }`

### agents.list
List all orchestrated agents.

**Params:** `{ task_id?: string, status?: string }`
**Returns:** `{ agents: AgentRecord[] }`

### agents.cancel
Cancel a running agent.

**Params:** `{ agent_id: string }`
**Returns:** `{ cancelled: true }`

### agents.heartbeat
Send a heartbeat for an agent.

**Params:** `{ agent_id: string }`
**Returns:** `{ ok: true }`

---

## analytics.*

### analytics.personal
Return personal productivity analytics for the current user.

**Params:** `{ days?: number }`
**Returns:** `PersonalAnalytics`

### analytics.providers
Return token usage breakdown by provider.

**Params:** `{ days?: number }`
**Returns:** `{ providers: ProviderUsage[] }`

### analytics.session
Return analytics for a specific session.

**Params:** `{ session_id: string }`
**Returns:** `SessionAnalytics`

### analytics.achievements
List developer achievements unlocked by the current user.

**Params:** none
**Returns:** `{ achievements: Achievement[] }`

---

## approval.*

### approval.list
List all tasks currently waiting for human approval.

**Params:** none
**Returns:** `{ approvals: ApprovalRequest[] }`

### approval.respond
Grant or deny a pending approval request.

**Params:** `{ approval_id: string, decision: "grant" | "deny", reason?: string }`
**Returns:** `{ approval_id: string, task_id: string, decision: string }`

---

## arena.*

### arena.create
Create an arena session to compare two providers on the same prompt.

**Params:** `{ session_id: string, provider_a: string, provider_b: string }`
**Returns:** `{ arena_id: string }`

### arena.vote
Record a user vote on an arena response.

**Params:** `{ arena_id: string, winner: "a" | "b" | "tie" }`
**Returns:** `{ recorded: true }`

### arena.leaderboard
Return the provider win/loss leaderboard.

**Params:** `{ limit?: number }`
**Returns:** `{ entries: LeaderboardEntry[] }`

---

## browser.*

### browser.screenshot
Take a screenshot of a URL using a headless browser.

**Params:** `{ url: string, width?: number, height?: number }`
**Returns:** `{ image_base64: string, content_type: string }`

---

## builder.*

### builder.create
Create a builder mode session for scaffolding new projects.

**Params:** `{ template: string, output_dir: string, vars?: Record<string, string> }`
**Returns:** `{ builder_id: string }`

### builder.templates
List available project templates.

**Params:** none
**Returns:** `{ templates: BuilderTemplate[] }`

### builder.status
Get the status of a builder session.

**Params:** `{ builder_id: string }`
**Returns:** `BuilderStatus`

---

## completion.*

### completion.suggest
Request inline code completion suggestions.

**Params:** `{ file_path: string, prefix: string, language?: string, max_suggestions?: number }`
**Returns:** `{ suggestions: CompletionSuggestion[] }`

---

## context.*

### context.bridge
Bridge context from one session to a new session.

**Params:** `{ source_session_id: string, target_session_id: string, summary?: boolean }`
**Returns:** `{ bridged: true, token_count: number }`

---

## daemon.*

### daemon.auth
**Required first message.** Authenticate the connection.

**Params:** `{ token: string }`
**Returns:** `{ authenticated: true }`

### daemon.ping
Check if the daemon is alive.

**Params:** none
**Returns:** `{ pong: true, version: string }`

### daemon.status
Return daemon status and configuration summary.

**Params:** none
**Returns:** `DaemonStatus`

### daemon.checkUpdate
Check whether a daemon update is available on GitHub Releases.

**Params:** none
**Returns:** `{ current: string, latest: string, update_available: boolean }`

### daemon.applyUpdate
Download and apply an available update (restarts daemon).

**Params:** none
**Returns:** `{ applying: true }`

### daemon.updatePolicy
Get the current auto-update policy.

**Params:** none
**Returns:** `{ policy: "auto" | "notify" | "off" }`

### daemon.setUpdatePolicy
Set the auto-update policy.

**Params:** `{ policy: "auto" | "notify" | "off" }`
**Returns:** `{ updated: true }`

### daemon.checkProvider
Verify that a provider CLI is installed and authenticated.

**Params:** `{ provider: string }`
**Returns:** `{ available: boolean, version?: string, error?: string }`

### daemon.providers
List all detected and configured providers.

**Params:** none
**Returns:** `{ providers: ProviderInfo[] }`

### daemon.setName
Set a human-readable name for this daemon instance.

**Params:** `{ name: string }`
**Returns:** `{ updated: true }`

### daemon.pairPin
Generate a short PIN for device pairing.

**Params:** none
**Returns:** `{ pin: string, expires_at: string }`

---

## device.*

### device.pair
Pair a new device using a PIN.

**Params:** `{ pin: string, device_name: string, device_type?: string }`
**Returns:** `{ device_id: string, token: string }`

### device.list
List all paired devices.

**Params:** none
**Returns:** `{ devices: PairedDevice[] }`

### device.revoke
Revoke a paired device's access.

**Params:** `{ device_id: string }`
**Returns:** `{ revoked: true }`

### device.rename
Rename a paired device.

**Params:** `{ device_id: string, name: string }`
**Returns:** `{ updated: true }`

---

## doctor.*

### doctor.scan
Run the full daemon health diagnostics (8 checks).

**Params:** none
**Returns:** `{ checks: DoctorCheck[], overall: "ok" | "warning" | "error" }`

### doctor.fix
Attempt to auto-fix a failing check.

**Params:** `{ check_id: string }`
**Returns:** `{ fixed: boolean, message: string }`

### doctor.approveRelease
Record that a release plan has been approved.

**Params:** `{ version: string }`
**Returns:** `{ approved: true }`

### doctor.hookInstall
Install the ClawDE CC pre-commit and session hooks.

**Params:** `{ repo_path?: string }`
**Returns:** `{ installed: string[] }`

---

## drift.*

### drift.scan
Run the drift scanner on a repository.

**Params:** `{ repo_path: string }`
**Returns:** `{ items: DriftItem[], score: number }`

### drift.list
List all drift items for a repository.

**Params:** `{ repo_path: string, limit?: number }`
**Returns:** `{ items: DriftItem[] }`

---

## ide.* (Sprint Z — IDE Extension Integration)

### ide.extensionConnected
Register that an IDE extension has connected to the daemon.

**Params:** `{ extensionType: "vscode" | "jetbrains" | "neovim" | "emacs", extensionVersion?: string }`
**Returns:** `{ connectionId: string }`

**Push event emitted:** `ide.extensionConnected`

### ide.editorContext
Push the current editor state from an IDE extension into the daemon.

**Params:**
```json
{
  "connectionId": "string",
  "extensionType": "string",
  "filePath": "string | null",
  "language": "string | null",
  "cursorLine": "number | null",
  "cursorCol": "number | null",
  "selectionText": "string | null",
  "visibleRangeStart": "number | null",
  "visibleRangeEnd": "number | null",
  "workspaceRoot": "string | null"
}
```
**Returns:** `{ stored: true }`
**Push event emitted:** `editor.contextChanged`

### ide.syncSettings
Push settings from the desktop app to all connected IDE extensions.

**Params:** any JSON settings object
**Returns:** `{ broadcast: true, extensionCount: number }`
**Push event emitted:** `settings.changed`

### ide.listConnections
List all currently-connected IDE extensions.

**Params:** none
**Returns:** `{ connections: IdeConnectionRecord[], count: number }`

### ide.latestContext
Return the most-recent editor context from any connected IDE.

**Params:** none
**Returns:** `EditorContext | null`

---

## license.*

### license.get
Return the full stored license info.

**Params:** none
**Returns:** `LicenseInfo`

### license.check
Check whether a specific feature is available on the current tier.

**Params:** `{ feature: string }`
**Returns:** `{ allowed: boolean, tier: string }`

### license.tier
Return the current tier string.

**Params:** none
**Returns:** `{ tier: string }`

---

## lsp.*

### lsp.start
Start an LSP server process for a language.

**Params:** `{ language: string, root_uri: string }`
**Returns:** `{ server_id: string }`

### lsp.stop
Stop a running LSP server.

**Params:** `{ server_id: string }`
**Returns:** `{ stopped: true }`

### lsp.diagnostics
Return current diagnostics from an LSP server.

**Params:** `{ server_id: string, file_uri?: string }`
**Returns:** `{ diagnostics: LspDiagnostic[] }`

### lsp.completions
Request completions from an LSP server.

**Params:** `{ server_id: string, file_uri: string, line: number, character: number }`
**Returns:** `{ items: LspCompletionItem[] }`

### lsp.list
List all running LSP servers.

**Params:** none
**Returns:** `{ servers: LspServerInfo[] }`

---

## mailbox.*

### mailbox.send
Send a cross-daemon inbox message.

**Params:** `{ to: string, subject: string, body: string, from?: string }`
**Returns:** `{ message_id: string }`

### mailbox.list
List inbox messages.

**Params:** `{ limit?: number, unread_only?: boolean }`
**Returns:** `{ messages: MailboxMessage[] }`

### mailbox.archive
Archive a mailbox message.

**Params:** `{ message_id: string }`
**Returns:** `{ archived: true }`

---

## message.*

### message.pin
Pin a message in a session.

**Params:** `{ session_id: string, message_id: string }`
**Returns:** `{ pinned: true }`

### message.unpin
Unpin a message.

**Params:** `{ session_id: string, message_id: string }`
**Returns:** `{ unpinned: true }`

---

## onboarding.*

### onboarding.checkAll
Run all provider checks and return a readiness summary.

**Params:** none
**Returns:** `{ providers: OnboardingCheck[] }`

### onboarding.checkProvider
Check a specific provider's readiness.

**Params:** `{ provider: string }`
**Returns:** `OnboardingCheck`

### onboarding.addApiKey
Store a provider API key and validate it.

**Params:** `{ provider: string, api_key: string }`
**Returns:** `{ valid: boolean }`

### onboarding.capabilities
Return the capabilities of a configured account.

**Params:** `{ provider: string }`
**Returns:** `{ models: string[], features: string[] }`

### onboarding.generateGci
Generate a `CLAUDE.md` (Global Claude Instructions) template.

**Params:** `{ project_name?: string }`
**Returns:** `{ content: string }`

### onboarding.generateCodexMd
Generate a `CODEX.md` template for OpenAI Codex.

**Params:** `{ project_name?: string }`
**Returns:** `{ content: string }`

### onboarding.generateCursorRules
Generate a `.cursorrules` file template.

**Params:** `{ project_name?: string }`
**Returns:** `{ content: string }`

### onboarding.bootstrapAid
Bootstrap the AID (AI Instruction Directory) for a workspace.

**Params:** `{ repo_path: string }`
**Returns:** `{ created: string[] }`

### onboarding.checkAid
Check whether the AID is properly set up.

**Params:** `{ repo_path: string }`
**Returns:** `{ valid: boolean, issues: string[] }`

---

## packs.*

### packs.install
Install a pack from the marketplace.

**Params:** `{ pack_id: string, version?: string }`
**Returns:** `{ installed: true }`

### packs.update
Update an installed pack.

**Params:** `{ pack_id: string }`
**Returns:** `{ updated_to: string }`

### packs.remove
Remove an installed pack.

**Params:** `{ pack_id: string }`
**Returns:** `{ removed: true }`

### packs.search
Search the pack marketplace.

**Params:** `{ query: string, limit?: number }`
**Returns:** `{ results: PackInfo[] }`

### packs.list
List all installed packs.

**Params:** none
**Returns:** `{ packs: InstalledPack[] }`

---

## project.*

### project.create
Create a new ClawDE project.

**Params:** `{ name: string, description?: string }`
**Returns:** `{ project_id: string }`

### project.list
List all projects.

**Params:** none
**Returns:** `{ projects: Project[] }`

### project.get
Get a project by ID.

**Params:** `{ project_id: string }`
**Returns:** `Project`

### project.update
Update project name or description.

**Params:** `{ project_id: string, name?: string, description?: string }`
**Returns:** `{ updated: true }`

### project.delete
Delete a project.

**Params:** `{ project_id: string }`
**Returns:** `{ deleted: true }`

### project.addRepo
Add a repository to a project.

**Params:** `{ project_id: string, repo_path: string }`
**Returns:** `{ added: true }`

### project.removeRepo
Remove a repository from a project.

**Params:** `{ project_id: string, repo_path: string }`
**Returns:** `{ removed: true }`

---

## prompt.*

### prompt.suggest
Get intelligent prompt suggestions based on the current context.

**Params:** `{ session_id: string, partial?: string }`
**Returns:** `{ suggestions: PromptSuggestion[] }`

### prompt.recordUsed
Record that a prompt suggestion was used.

**Params:** `{ suggestion_id: string }`
**Returns:** `{ recorded: true }`

---

## providers.*

### providers.detect
Auto-detect all installed AI provider CLIs.

**Params:** none
**Returns:** `{ detected: string[] }`

### providers.list
List all known providers with installation status.

**Params:** none
**Returns:** `{ providers: ProviderInfo[] }`

---

## repo.*

### repo.list
List all open repositories.

**Params:** none
**Returns:** `{ repos: RepoInfo[] }`

### repo.open
Open a repository by path.

**Params:** `{ path: string }`
**Returns:** `{ repo_id: string }`

### repo.close
Close an open repository.

**Params:** `{ repo_id: string }`
**Returns:** `{ closed: true }`

### repo.status
Return git status for a repository.

**Params:** `{ repo_id: string }`
**Returns:** `RepoStatus`

### repo.diff
Return the full diff for a repository.

**Params:** `{ repo_id: string, staged?: boolean }`
**Returns:** `{ diff: string }`

### repo.fileDiff
Return the diff for a single file.

**Params:** `{ repo_id: string, file_path: string, staged?: boolean }`
**Returns:** `{ diff: string }`

### repo.tree
Return the file tree for a repository.

**Params:** `{ repo_id: string, max_depth?: number }`
**Returns:** `{ tree: FileTreeNode[] }`

### repo.readFile
Read the contents of a file in a repository.

**Params:** `{ repo_id: string, file_path: string }`
**Returns:** `{ content: string, size: number }`

### repo.scan / repo.profile / repo.generateArtifacts / repo.syncArtifacts / repo.driftScore / repo.driftReport
See [Repo Intelligence](Features/Repo-Intelligence.md).

---

## review.*

### review.run
Run an AI code review on a diff or file set.

**Params:** `{ repo_id: string, diff?: string, files?: string[] }`
**Returns:** `{ review_id: string, issues: ReviewIssue[] }`

### review.fix
Apply a suggested fix from a code review.

**Params:** `{ review_id: string, issue_id: string }`
**Returns:** `{ fixed: true }`

### review.learn
Record that a review finding was accepted/rejected (training signal).

**Params:** `{ review_id: string, issue_id: string, accepted: boolean }`
**Returns:** `{ recorded: true }`

---

## session.*

### session.create
Create a new AI session.

**Params:** `{ provider: string, repo_path?: string, title?: string, mode?: string }`
**Returns:** `Session`

### session.list
List all sessions.

**Params:** `{ status?: string, limit?: number }`
**Returns:** `{ sessions: Session[] }`

### session.get
Get a session by ID.

**Params:** `{ session_id: string }`
**Returns:** `Session`

### session.delete
Delete a session.

**Params:** `{ session_id: string }`
**Returns:** `{ deleted: true }`

### session.sendMessage
Send a message to a session and start a provider turn.

**Params:** `{ session_id: string, content: string }`
**Returns:** `{ message_id: string }`

### session.getMessages
Return the message history for a session.

**Params:** `{ session_id: string, limit?: number, before?: string }`
**Returns:** `{ messages: Message[] }`

### session.pause
Pause a session (suspends the provider process).

**Params:** `{ session_id: string }`
**Returns:** `{ paused: true }`

### session.resume
Resume a paused session.

**Params:** `{ session_id: string }`
**Returns:** `{ resumed: true }`

### session.cancel
Cancel the currently-running provider turn.

**Params:** `{ session_id: string }`
**Returns:** `{ cancelled: true }`

### session.setProvider
Change the AI provider for a session.

**Params:** `{ session_id: string, provider: string }`
**Returns:** `{ updated: true }`

### session.setMode
Set the GCI mode for a session.

**Params:** `{ session_id: string, mode: "NORMAL" | "LEARN" | "STORM" | "FORGE" | "CRUNCH" }`
**Returns:** `{ updated: true }`

### session.setModel
Set an explicit model override for a session.

**Params:** `{ session_id: string, model: string }`
**Returns:** `{ updated: true }`

### session.toolCallAudit
Query the tool call audit log for a session.

**Params:** `{ session_id?: string, limit?: number, before?: string }`
**Returns:** `{ events: ToolCallEvent[], count: number }`

---

## tasks.*

See [Tasks](Tasks/Overview.md) for the full task system reference.

Key methods: `tasks.list`, `tasks.get`, `tasks.addTask`, `tasks.bulkAdd`, `tasks.claim`, `tasks.release`, `tasks.updateStatus`, `tasks.heartbeat`, `tasks.logActivity`, `tasks.note`, `tasks.activity`, `tasks.fromPlanning`, `tasks.fromChecklist`, `tasks.summary`, `tasks.progressEstimate`, `tasks.export`, `tasks.validate`, `tasks.sync`, `tasks.createSpec`, `tasks.transition`, `tasks.listEvents`.

---

## te.* (Task Engine — Phase 45)

Low-level task engine with explicit phase/agent management. See [Tasks/Task-Engine.md](Tasks/Task-Engine.md).

Methods: `te.phase.create`, `te.phase.list`, `te.task.create`, `te.task.get`, `te.task.list`, `te.task.transition`, `te.task.claim`, `te.agent.register`, `te.agent.heartbeat`, `te.agent.deregister`, `te.event.log`, `te.event.list`, `te.checkpoint.write`, `te.note.add`, `te.note.list`.

---

## threads.*

### threads.start
Start a new conversation thread.

**Params:** `{ session_id: string, title?: string }`
**Returns:** `{ thread_id: string }`

### threads.resume
Resume an existing thread.

**Params:** `{ thread_id: string }`
**Returns:** `{ resumed: true }`

### threads.fork
Fork a thread from a specific message.

**Params:** `{ thread_id: string, from_message_id: string }`
**Returns:** `{ new_thread_id: string }`

### threads.list
List all threads for a session.

**Params:** `{ session_id: string }`
**Returns:** `{ threads: Thread[] }`

---

## token.*

### token.sessionUsage
Return token usage for a session.

**Params:** `{ session_id: string }`
**Returns:** `TokenUsage`

### token.totalUsage
Return total token usage across all sessions.

**Params:** `{ days?: number }`
**Returns:** `AggregateTokenUsage`

### token.budgetStatus
Return current budget status for the user.

**Params:** none
**Returns:** `BudgetStatus`

---

## tool.*

### tool.approve
Approve a pending tool call.

**Params:** `{ session_id: string, tool_call_id: string }`
**Returns:** `{ approved: true }`

### tool.reject
Reject a pending tool call.

**Params:** `{ session_id: string, tool_call_id: string, reason?: string }`
**Returns:** `{ rejected: true }`

---

## topology.*

### topology.get
Return the dependency topology for a set of repos.

**Params:** `{ repo_paths: string[] }`
**Returns:** `Topology`

### topology.validate
Validate topology for circular or broken dependencies.

**Params:** `{ repo_paths: string[] }`
**Returns:** `{ valid: boolean, issues: string[] }`

### topology.addDependency
Add a dependency edge between two repos.

**Params:** `{ from: string, to: string }`
**Returns:** `{ added: true }`

### topology.removeDependency
Remove a dependency edge.

**Params:** `{ from: string, to: string }`
**Returns:** `{ removed: true }`

### topology.crossValidate
Cross-validate task plans across repo boundaries.

**Params:** `{ repo_paths: string[] }`
**Returns:** `CrossValidationResult`

---

## traces.*

### traces.query
Query observability traces.

**Params:** `{ session_id?: string, from?: string, to?: string, limit?: number }`
**Returns:** `{ traces: Trace[] }`

### traces.summary
Return a cost/token summary of traces.

**Params:** `{ days?: number }`
**Returns:** `TracesSummary`

---

## validators.*

### validators.list
List all available validators for a repo.

**Params:** `{ repo_path: string }`
**Returns:** `{ validators: ValidatorInfo[] }`

### validators.run
Run a specific validator.

**Params:** `{ repo_path: string, validator_id: string }`
**Returns:** `ValidatorResult`

---

## worktrees.*

### worktrees.create
Create a git worktree for a task.

**Params:** `{ task_id: string, repo_path: string, base_branch?: string }`
**Returns:** `{ worktree_path: string, branch: string }`

### worktrees.list
List all task worktrees.

**Params:** `{ repo_path?: string }`
**Returns:** `{ worktrees: WorktreeInfo[] }`

### worktrees.diff
Return the diff for a worktree.

**Params:** `{ task_id: string }`
**Returns:** `{ diff: string }`

### worktrees.commit
Commit staged changes in a worktree.

**Params:** `{ task_id: string, message: string }`
**Returns:** `{ commit_sha: string }`

### worktrees.accept
Accept worktree changes and merge to the base branch.

**Params:** `{ task_id: string }`
**Returns:** `{ merged: true }`

### worktrees.reject
Reject worktree changes and discard them.

**Params:** `{ task_id: string }`
**Returns:** `{ rejected: true }`

### worktrees.delete
Delete a worktree.

**Params:** `{ task_id: string }`
**Returns:** `{ deleted: true }`

### worktrees.merge
Merge a worktree branch without deleting the worktree.

**Params:** `{ task_id: string, strategy?: string }`
**Returns:** `{ merged: true }`

### worktrees.cleanup
Remove all orphaned (task-deleted) worktrees.

**Params:** none
**Returns:** `{ removed: number }`

---

## Push events

The daemon broadcasts events to all connected clients without a request:

| Event | Trigger |
| --- | --- |
| `daemon.ready` | Daemon started |
| `session.statusChanged` | Session status changes |
| `session.messageAdded` | New message in a session |
| `session.turnStarted` | Provider turn started |
| `session.turnCompleted` | Provider turn finished |
| `session.toolCallRequested` | Tool call awaiting approval |
| `task.statusChanged` | Task status changes |
| `task.approvalGranted` | Approval granted |
| `task.approvalDenied` | Approval denied |
| `warning.versionBump` | Version file changed in a monitored repo |
| `ide.extensionConnected` | IDE extension connected (Sprint Z) |
| `editor.contextChanged` | Editor context updated (Sprint Z) |
| `settings.changed` | Settings synced to extensions (Sprint Z) |

---

*This reference is generated from `apps/daemon/src/ipc/mod.rs` dispatch table (174 methods).*
