//! Integration tests for Phase 43e: Multi-Agent Orchestration.
//!
//! Tests the orchestrator, agent lifecycle registry, and IPC handlers.

use chrono::Utc;
use clawd::agents::{
    capabilities::Provider,
    lifecycle::{AgentRegistry, AgentStatus},
    orchestrator::Orchestrator,
    roles::AgentRole,
};

// ─── 43e-17.1: Spawn a Router agent ─────────────────────────────────────────

#[tokio::test]
async fn test_spawn_router_agent() {
    let orch = Orchestrator::new();

    let agent_id = orch
        .spawn(AgentRole::Router, "task-001", "low", None, None)
        .await
        .expect("spawn should succeed");

    assert!(agent_id.starts_with("A-"), "agent id should start with A-");

    let registry = orch.registry.read().await;
    let record = registry.get(&agent_id).expect("record should exist");
    assert_eq!(record.role, AgentRole::Router);
    assert_eq!(record.task_id, "task-001");
    assert_eq!(record.status, AgentStatus::Pending);
}

// ─── 43e-17.2: Concurrency cap enforcement ───────────────────────────────────

#[tokio::test]
async fn test_concurrency_cap() {
    let orch = Orchestrator::new();
    let task_id = "task-cap-test";

    // Implementer max_concurrent = 3
    for i in 0..3 {
        orch.spawn(AgentRole::Implementer, task_id, "medium", None, None)
            .await
            .unwrap_or_else(|e| panic!("spawn {} should succeed: {}", i, e));
    }

    // 4th spawn must fail with ConcurrencyCapReached
    let result = orch
        .spawn(AgentRole::Implementer, task_id, "medium", None, None)
        .await;

    assert!(result.is_err(), "4th implementer spawn should be rejected");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("concurrency cap"),
        "error should mention concurrency cap, got: {}",
        err_msg
    );
}

// ─── 43e-17.3: Cross-model reviewer ──────────────────────────────────────────

#[tokio::test]
async fn test_cross_model_reviewer() {
    let orch = Orchestrator::new();
    let task_id = "task-crossmodel";

    // Spawn implementer — should use Claude (default for implementer role)
    let impl_id = orch
        .spawn(AgentRole::Implementer, task_id, "medium", None, None)
        .await
        .expect("implementer spawn");

    let registry = orch.registry.read().await;
    let impl_record = registry.get(&impl_id).expect("impl record");
    let impl_provider = impl_record.provider.clone();
    drop(registry);

    // Spawn reviewer with previous_provider = implementer's provider
    // Cross-model rule: reviewer should be different from implementer
    let review_id = orch
        .spawn(
            AgentRole::Reviewer,
            task_id,
            "medium",
            None,
            Some(impl_provider.clone()),
        )
        .await
        .expect("reviewer spawn");

    let registry = orch.registry.read().await;
    let review_record = registry.get(&review_id).expect("reviewer record");

    // If implementer used Claude, reviewer must use Codex (and vice versa)
    match &impl_provider {
        Provider::Claude => {
            assert_eq!(
                review_record.provider,
                Provider::Codex,
                "reviewer should use Codex when implementer used Claude"
            );
        }
        Provider::Codex => {
            assert_eq!(
                review_record.provider,
                Provider::Claude,
                "reviewer should use Claude when implementer used Codex"
            );
        }
        _ => {}
    }
}

// ─── 43e-17.4: Heartbeat crash detection ─────────────────────────────────────

#[tokio::test]
async fn test_heartbeat_crash_detection() {
    let mut registry = AgentRegistry::new();

    // Manually insert an agent with an old heartbeat
    let stale_time = Utc::now() - chrono::Duration::seconds(120);
    let record = clawd::agents::lifecycle::AgentRecord {
        agent_id: "A-stale01".to_string(),
        role: AgentRole::Implementer,
        task_id: "task-crash".to_string(),
        provider: Provider::Claude,
        model: "claude-sonnet-4-6".to_string(),
        worktree_path: None,
        status: AgentStatus::Running,
        created_at: stale_time,
        last_heartbeat: stale_time,
        tokens_used: 0,
        cost_usd_est: 0.0,
        result: None,
        error: None,
    };
    registry.register(record);

    // Also add a fresh agent that should NOT be marked crashed
    let fresh_record = clawd::agents::lifecycle::AgentRecord {
        agent_id: "A-fresh01".to_string(),
        role: AgentRole::Planner,
        task_id: "task-crash".to_string(),
        provider: Provider::Claude,
        model: "claude-sonnet-4-6".to_string(),
        worktree_path: None,
        status: AgentStatus::Running,
        created_at: Utc::now(),
        last_heartbeat: Utc::now(),
        tokens_used: 0,
        cost_usd_est: 0.0,
        result: None,
        error: None,
    };
    registry.register(fresh_record);

    // Detect crashed with 60s timeout — stale agent (120s old) should be caught
    let crashed = registry.detect_crashed(60);

    assert_eq!(crashed.len(), 1, "exactly one agent should be crashed");
    assert_eq!(crashed[0], "A-stale01");

    // Verify status was updated
    let stale = registry
        .get("A-stale01")
        .expect("stale record should exist");
    assert_eq!(stale.status, AgentStatus::Crashed);

    // Fresh agent should still be Running
    let fresh = registry
        .get("A-fresh01")
        .expect("fresh record should exist");
    assert_eq!(fresh.status, AgentStatus::Running);
}

// ─── 43e-17.5: Handoff chain ─────────────────────────────────────────────────

#[tokio::test]
async fn test_handoff_chain() {
    let orch = Orchestrator::new();

    let chain = orch
        .run_handoff_chain("task-chain-01", "medium")
        .await
        .expect("handoff chain should succeed");

    assert_eq!(chain.len(), 4, "chain should have 4 agents");

    let registry = orch.registry.read().await;

    let planner = registry.get(&chain[0]).expect("planner record");
    assert_eq!(planner.role, AgentRole::Planner);

    let implementer = registry.get(&chain[1]).expect("implementer record");
    assert_eq!(implementer.role, AgentRole::Implementer);

    let reviewer = registry.get(&chain[2]).expect("reviewer record");
    assert_eq!(reviewer.role, AgentRole::Reviewer);
    // Cross-model: reviewer should differ from implementer
    assert_ne!(
        reviewer.provider, implementer.provider,
        "reviewer must use a different provider than implementer"
    );

    let qa = registry.get(&chain[3]).expect("qa record");
    assert_eq!(qa.role, AgentRole::QaExecutor);
}
