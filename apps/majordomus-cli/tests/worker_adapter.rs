//! A worker adapter runs in the execution's worktree and streams into its events (I3982,
//! ADR 0225). Every case uses a stand-in program, never a provider.

use std::sync::Arc;
use std::time::{Duration, Instant};

use majordomus_cli::execution::{
    Actor, ActorKind, EventPayload, ExecutionId, ExecutionStore, Limits, Progress, RepositoryRef,
};
use majordomus_cli::worker::{choose, run, Adapter, Assignment, Outcome, Program};

fn execution() -> (Arc<ExecutionStore>, ExecutionId, Progress) {
    let store = Arc::new(ExecutionStore::new(Limits::default()));
    let id = ExecutionId::fresh();
    let cancel = store.create(
        id.clone(),
        "issue.implement",
        "Implement",
        serde_json::json!({}),
        true,
        Actor::of(ActorKind::Internal),
        RepositoryRef {
            name: "r".into(),
            id: "id".into(),
            branch: None,
        },
    );
    store.publish(&id, EventPayload::Started);
    let progress = Progress::reporting(Arc::clone(&store), id.clone(), cancel);
    (store, id, progress)
}

fn logs(store: &ExecutionStore, id: &ExecutionId) -> Vec<String> {
    store
        .events(id, 0, 10_000)
        .unwrap()
        .events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::Log { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn sh(script: &str) -> Program {
    Program::new("fixture", "sh", &["-c", script, "worker"])
}

#[test]
fn the_workers_lines_arrive_as_the_executions_events_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, progress) = execution();
    let outcome = run(
        &sh("echo one; echo two; echo three"),
        &Assignment::new(id.as_str(), dir.path(), "count"),
        &progress,
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Succeeded);
    assert_eq!(logs(&store, &id), ["one", "two", "three"]);
    let sequences: Vec<u64> = store
        .events(&id, 0, 10_000)
        .unwrap()
        .events
        .iter()
        .map(|e| e.sequence)
        .collect();
    assert!(
        sequences.windows(2).all(|w| w[1] == w[0] + 1),
        "dense: {sequences:?}"
    );
}

#[test]
fn the_worker_runs_in_the_worktree_and_knows_its_execution() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, progress) = execution();
    run(
        &sh("pwd -P; echo \"$MAJORDOMUS_EXECUTION_ID\"; echo done > made-here"),
        &Assignment::new(id.as_str(), dir.path(), "where"),
        &progress,
    )
    .unwrap();
    let lines = logs(&store, &id);
    assert_eq!(
        std::path::Path::new(&lines[0]),
        dir.path().canonicalize().unwrap()
    );
    assert_eq!(lines[1], id.as_str());
    assert!(
        dir.path().join("made-here").exists(),
        "the change is in the worktree"
    );
}

#[test]
fn a_failing_worker_says_how_it_exited() {
    let dir = tempfile::tempdir().unwrap();
    let (_, id, progress) = execution();
    let outcome = run(
        &sh("echo nope >&2; exit 3"),
        &Assignment::new(id.as_str(), dir.path(), "fail"),
        &progress,
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Failed { exit: Some(3) });
}

#[test]
fn the_deadline_stops_the_worker_and_everything_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let (_, id, progress) = execution();
    let started = Instant::now();
    let outcome = run(
        &sh("sleep 30 & sleep 30; wait"),
        &Assignment::new(id.as_str(), dir.path(), "hang").within(Duration::from_millis(300)),
        &progress,
    )
    .unwrap();
    assert_eq!(outcome, Outcome::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "not waited for"
    );
}

#[test]
fn a_cancelled_execution_stops_its_worker() {
    let dir = tempfile::tempdir().unwrap();
    let (store, id, progress) = execution();
    let canceller = {
        let store = Arc::clone(&store);
        let id = id.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            store.request_cancel(&id, "test");
        })
    };
    let outcome = run(
        &sh("sleep 30"),
        &Assignment::new(id.as_str(), dir.path(), "wait"),
        &progress,
    )
    .unwrap();
    canceller.join().unwrap();
    assert_eq!(outcome, Outcome::Cancelled);
}

#[test]
fn with_no_adapter_available_every_reason_is_given() {
    let adapters: Vec<Box<dyn Adapter>> = vec![
        Box::new(Program::new("one", "no-such-program-one", &[])),
        Box::new(Program::new("two", "no-such-program-two", &[])),
    ];
    let reasons = choose(&adapters).unwrap_err();
    assert_eq!(
        reasons
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert!(reasons[0].1.contains("not on PATH"));
}
