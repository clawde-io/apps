//! Sprint CC CA.8 — Automation engine tests.

use clawd::automations::{
    builtins,
    engine::{Automation, AutomationEngine, TriggerEvent, TriggerType},
};

fn make_test_engine() -> std::sync::Arc<AutomationEngine> {
    AutomationEngine::new(builtins::all())
}

#[test]
fn builtin_automations_are_registered() {
    let engine = make_test_engine();
    let autos = engine.automations.blocking_read();
    assert!(!autos.is_empty(), "should have at least the 3 built-ins");
    let names: Vec<&str> = autos.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"run-tests-on-complete"));
    assert!(names.contains(&"todo-extractor"));
    assert!(names.contains(&"long-session-notifier"));
}

#[test]
fn disabled_automation_does_not_match() {
    let engine = make_test_engine();
    let autos = engine.automations.blocking_read();
    let run_tests = autos
        .iter()
        .find(|a| a.name == "run-tests-on-complete")
        .unwrap();

    // run-tests-on-complete is disabled by default.
    let event = TriggerEvent {
        kind: TriggerType::SessionComplete,
        session_id: Some("s1".into()),
        task_id: None,
        file_path: None,
        session_output: None,
        session_duration_secs: None,
    };
    assert!(!run_tests.matches(&event), "disabled automation should not match");
}

#[test]
fn todo_extractor_matches_session_complete() {
    let engine = make_test_engine();
    let autos = engine.automations.blocking_read();
    let extractor = autos.iter().find(|a| a.name == "todo-extractor").unwrap();
    assert!(extractor.enabled, "todo-extractor should be enabled by default");

    let event = TriggerEvent {
        kind: TriggerType::SessionComplete,
        session_id: Some("s1".into()),
        task_id: None,
        file_path: None,
        session_output: Some("TODO: Fix the login bug".into()),
        session_duration_secs: None,
    };
    assert!(extractor.matches(&event));
}

#[test]
fn long_session_notifier_condition_filtering() {
    let engine = make_test_engine();
    let autos = engine.automations.blocking_read();
    let notifier = autos
        .iter()
        .find(|a| a.name == "long-session-notifier")
        .unwrap();

    // Short session: should NOT match.
    let short_event = TriggerEvent {
        kind: TriggerType::SessionComplete,
        session_id: Some("s2".into()),
        task_id: None,
        file_path: None,
        session_output: None,
        session_duration_secs: Some(60),
    };
    assert!(!notifier.matches(&short_event), "60s session should not trigger notifier");

    // Long session: should match.
    let long_event = TriggerEvent {
        kind: TriggerType::SessionComplete,
        session_id: Some("s3".into()),
        task_id: None,
        file_path: None,
        session_output: None,
        session_duration_secs: Some(400),
    };
    assert!(notifier.matches(&long_event), "400s session should trigger notifier");
}

#[test]
fn wrong_trigger_type_does_not_match() {
    let engine = make_test_engine();
    let autos = engine.automations.blocking_read();
    let extractor = autos.iter().find(|a| a.name == "todo-extractor").unwrap();

    let wrong_event = TriggerEvent {
        kind: TriggerType::TaskDone, // Wrong trigger type
        session_id: None,
        task_id: Some("t1".into()),
        file_path: None,
        session_output: None,
        session_duration_secs: None,
    };
    assert!(!extractor.matches(&wrong_event));
}

#[test]
fn fire_does_not_panic_without_listeners() {
    let engine = make_test_engine();
    // Fire without starting the dispatcher — should not panic.
    engine.fire(TriggerEvent {
        kind: TriggerType::SessionComplete,
        session_id: Some("s1".into()),
        task_id: None,
        file_path: None,
        session_output: None,
        session_duration_secs: Some(10),
    });
}
