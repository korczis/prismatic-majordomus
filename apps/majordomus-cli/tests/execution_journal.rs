//! An execution outlives the server that accepted it (I3980, ADR 0222).
//!
//! Each case builds a journaled store, lets it go as a stopped server would, and builds a
//! second one over the same directory: what the second answers is what a browser that
//! reconnects after a restart is shown.

use majordomus_cli::execution::{
    Actor, ActorKind, EventPayload, ExecutionId, ExecutionJournal, ExecutionState, ExecutionStore,
    Limits, RepositoryRef, Seed,
};

fn repository() -> RepositoryRef {
    RepositoryRef {
        name: "r".into(),
        id: "id".into(),
        branch: None,
    }
}

fn server(dir: &std::path::Path, limits: Limits) -> ExecutionStore {
    ExecutionStore::journaled(limits, ExecutionJournal::at(dir).unwrap())
}

fn start(store: &ExecutionStore) -> ExecutionId {
    let id = ExecutionId::fresh();
    store.create(
        id.clone(),
        "executions.demonstrate",
        "Demonstrate an execution",
        serde_json::json!({ "steps": 3 }),
        true,
        Actor::of(ActorKind::Http),
        repository(),
    );
    id
}

#[test]
fn an_execution_running_when_its_server_stopped_comes_back_interrupted_and_is_not_run_again() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let before = server(dir.path(), Limits::default());
        let id = start(&before);
        before.publish(&id, EventPayload::Started);
        id
    };
    let after = server(dir.path(), Limits::default());
    let recovered = after.get(&id).expect("the execution is still known");
    assert_eq!(recovered.state, ExecutionState::Failed);
    let error = recovered.error.expect("it says why it ended");
    assert_eq!(error.code, "interrupted");
    assert!(error.message.contains("sequence 2"), "{}", error.message);
    // a third start of the store recovers the same failure and adds nothing
    drop(after);
    let again = server(dir.path(), Limits::default());
    assert_eq!(
        again.get(&id).unwrap().last_sequence,
        recovered.last_sequence
    );
}

#[test]
fn the_cursor_replays_exactly_the_gap_across_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (id, seen) = {
        let before = server(dir.path(), Limits::default());
        let id = start(&before);
        before.publish(&id, EventPayload::Started);
        before.publish(
            &id,
            EventPayload::Completed {
                output: serde_json::json!({ "ok": true }),
            },
        );
        // a browser saw up to the first event before the restart
        (id, 1u64)
    };
    let after = server(dir.path(), Limits::default());
    let page = after.events(&id, seen, 100).unwrap();
    let sequences: Vec<u64> = page.events.iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![2, 3], "the gap and nothing else");
    assert!(!page.truncated);
    let finished = after.get(&id).unwrap();
    assert_eq!(
        finished.state,
        ExecutionState::Succeeded,
        "a finished one stays as it was"
    );
    assert_eq!(finished.output, Some(serde_json::json!({ "ok": true })));
}

#[test]
fn a_reported_execution_is_reattached_when_its_reporter_reports_again() {
    let dir = tempfile::tempdir().unwrap();
    let id = ExecutionId::fresh();
    let seed = || Seed {
        capability: "executions.demonstrate",
        title: "Demonstrate an execution",
        input: serde_json::json!({ "steps": 3 }),
        cancellable: true,
        actor: Actor::reported(4242),
        repository: repository(),
    };
    {
        let before = server(dir.path(), Limits::default());
        before.adopt(id.clone(), seed(), 4242).unwrap();
        before
            .report(&id, vec![(2, EventPayload::Started)])
            .unwrap();
    }
    let after = server(dir.path(), Limits::default());
    assert!(
        after.is_reported(&id),
        "recovered as reported, not interrupted"
    );
    assert_eq!(after.get(&id).unwrap().state, ExecutionState::Running);
    // the reporter, still alive, continues where it was
    let outcome = after
        .report(
            &id,
            vec![(
                3,
                EventPayload::Completed {
                    output: serde_json::json!(1),
                },
            )],
        )
        .unwrap();
    assert_eq!((outcome.applied, outcome.through), (1, 3));
    assert_eq!(after.get(&id).unwrap().state, ExecutionState::Succeeded);
}

#[test]
fn a_forgotten_execution_is_archived_never_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        max_executions: 1,
        ..Limits::default()
    };
    let store = server(dir.path(), limits);
    let first = start(&store);
    store.publish(&first, EventPayload::Started);
    store.publish(
        &first,
        EventPayload::Completed {
            output: serde_json::json!(1),
        },
    );
    let second = start(&store);
    assert!(
        store.get(&first).is_none(),
        "the bound forgot the finished one"
    );
    assert!(store.get(&second).is_some());
    assert!(dir
        .path()
        .join("archive")
        .join(format!("{}.jsonl", first.as_str()))
        .exists());
}
