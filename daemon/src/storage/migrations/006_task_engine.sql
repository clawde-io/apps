-- Phase 45: Task Engine Data Model

-- Agents (must be created before te_tasks since te_tasks references te_agents)
CREATE TABLE IF NOT EXISTS te_agents (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  agent_type TEXT NOT NULL CHECK(agent_type IN ('claude','codex','chatgpt','cursor','human')),
  role TEXT NOT NULL DEFAULT 'implementer' CHECK(role IN ('router','planner','implementer','reviewer','qa','general')),
  session_id TEXT,
  connection_type TEXT NOT NULL DEFAULT 'local' CHECK(connection_type IN ('local','relay','remote')),
  status TEXT NOT NULL DEFAULT 'idle' CHECK(status IN ('idle','working','paused','disconnected','terminated')),
  current_task_id TEXT,
  last_heartbeat_at INTEGER NOT NULL DEFAULT (unixepoch()),
  heartbeat_interval_secs INTEGER NOT NULL DEFAULT 30,
  heartbeat_timeout_secs INTEGER NOT NULL DEFAULT 90,
  capabilities TEXT NOT NULL DEFAULT '[]',
  max_context_tokens INTEGER,
  model_id TEXT,
  tasks_completed INTEGER NOT NULL DEFAULT 0,
  tasks_failed INTEGER NOT NULL DEFAULT 0,
  total_tokens_used INTEGER NOT NULL DEFAULT 0,
  avg_task_duration_secs INTEGER,
  registered_at INTEGER NOT NULL DEFAULT (unixepoch()),
  last_active_at INTEGER,
  metadata TEXT
);
CREATE INDEX IF NOT EXISTS idx_te_agents_status ON te_agents(status);

-- Phases (groups of related tasks)
CREATE TABLE IF NOT EXISTS te_phases (
  id TEXT PRIMARY KEY,
  display_id TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'planned' CHECK(status IN ('planned','active','completed','canceled')),
  planning_doc_path TEXT,
  repo TEXT,
  priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('critical','high','medium','low')),
  created_at INTEGER NOT NULL DEFAULT (unixepoch()),
  started_at INTEGER,
  completed_at INTEGER,
  metadata TEXT
);

-- Tasks (work items)
CREATE TABLE IF NOT EXISTS te_tasks (
  id TEXT PRIMARY KEY,
  display_id TEXT NOT NULL UNIQUE,
  phase_id TEXT NOT NULL REFERENCES te_phases(id),
  parent_task_id TEXT REFERENCES te_tasks(id),
  depth INTEGER NOT NULL DEFAULT 0,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  ai_instructions TEXT NOT NULL DEFAULT '',
  requirements TEXT NOT NULL DEFAULT '[]',
  definition_of_done TEXT NOT NULL DEFAULT '[]',
  cr_checklist TEXT,
  qa_checklist TEXT,
  task_type TEXT NOT NULL DEFAULT 'implementation' CHECK(task_type IN ('implementation','bugfix','refactor','test','documentation','investigation','review','qa','secondary','tertiary','auxiliary','hotfix')),
  priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('critical','high','medium','low')),
  risk_level TEXT NOT NULL DEFAULT 'low' CHECK(risk_level IN ('low','medium','high','critical')),
  status TEXT NOT NULL DEFAULT 'planned' CHECK(status IN ('planned','ready','queued','claimed','active','paused','blocked','needs_review','in_review','review_failed','needs_qa','in_qa','qa_failed','needs_secondary','done','canceled','failed')),
  blocked_reason TEXT,
  pause_reason TEXT,
  failure_reason TEXT,
  claimed_by TEXT REFERENCES te_agents(id),
  claimed_at INTEGER,
  reviewer_agent_id TEXT REFERENCES te_agents(id),
  qa_agent_id TEXT REFERENCES te_agents(id),
  estimated_tokens INTEGER,
  estimated_minutes INTEGER,
  estimated_files INTEGER,
  repo TEXT,
  target_files TEXT,
  worktree_path TEXT,
  branch_name TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  max_retries INTEGER NOT NULL DEFAULT 3,
  created_at INTEGER NOT NULL DEFAULT (unixepoch()),
  started_at INTEGER,
  completed_at INTEGER,
  discovered_from_task_id TEXT REFERENCES te_tasks(id),
  tags TEXT,
  metadata TEXT
);
CREATE INDEX IF NOT EXISTS idx_te_tasks_phase ON te_tasks(phase_id);
CREATE INDEX IF NOT EXISTS idx_te_tasks_status ON te_tasks(status);
CREATE INDEX IF NOT EXISTS idx_te_tasks_priority_status ON te_tasks(priority, status);
CREATE INDEX IF NOT EXISTS idx_te_tasks_claimed_by ON te_tasks(claimed_by);

-- Task dependencies (DAG)
CREATE TABLE IF NOT EXISTS te_task_dependencies (
  task_id TEXT NOT NULL REFERENCES te_tasks(id),
  depends_on_task_id TEXT NOT NULL REFERENCES te_tasks(id),
  dependency_type TEXT NOT NULL DEFAULT 'blocks' CHECK(dependency_type IN ('blocks','soft','informs')),
  PRIMARY KEY (task_id, depends_on_task_id)
);

-- Events (append-only audit log)
CREATE TABLE IF NOT EXISTS te_events (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES te_tasks(id),
  agent_id TEXT REFERENCES te_agents(id),
  event_seq INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT UNIQUE,
  timestamp INTEGER NOT NULL DEFAULT (unixepoch()),
  UNIQUE(task_id, event_seq)
);
CREATE INDEX IF NOT EXISTS idx_te_events_task ON te_events(task_id, event_seq);
CREATE INDEX IF NOT EXISTS idx_te_events_type ON te_events(event_type);
CREATE INDEX IF NOT EXISTS idx_te_events_ts ON te_events(timestamp);

-- Notes
CREATE TABLE IF NOT EXISTS te_notes (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES te_tasks(id),
  agent_id TEXT REFERENCES te_agents(id),
  note_type TEXT NOT NULL CHECK(note_type IN ('discovery','decision','blocker','workaround','question','answer','observation','warning','performance','security','debt','idea','context')),
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  related_file TEXT,
  related_line INTEGER,
  visibility TEXT NOT NULL DEFAULT 'team' CHECK(visibility IN ('agent_only','team','public')),
  timestamp INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX IF NOT EXISTS idx_te_notes_task ON te_notes(task_id);

-- Checkpoints
CREATE TABLE IF NOT EXISTS te_checkpoints (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES te_tasks(id),
  agent_id TEXT NOT NULL REFERENCES te_agents(id),
  checkpoint_type TEXT NOT NULL CHECK(checkpoint_type IN ('periodic','milestone','pre_pause','pre_crash','handoff')),
  completed_items TEXT NOT NULL DEFAULT '[]',
  files_modified TEXT NOT NULL DEFAULT '[]',
  tests_run TEXT,
  builds_run TEXT,
  current_action TEXT NOT NULL DEFAULT '',
  current_file TEXT,
  partial_work TEXT,
  next_steps TEXT NOT NULL DEFAULT '[]',
  remaining_items TEXT NOT NULL DEFAULT '[]',
  key_discoveries TEXT,
  decisions_made TEXT,
  gotchas TEXT,
  patterns_observed TEXT,
  context_summary TEXT,
  environment_state TEXT,
  last_event_seq INTEGER NOT NULL DEFAULT 0,
  timestamp INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX IF NOT EXISTS idx_te_checkpoints_task ON te_checkpoints(task_id, timestamp)
