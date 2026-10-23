//! `implement.issue` judges readiness, leases the scope, creates the worktree and hands the
//! work to a worker, each a step of one execution (I3983, ADR 0225). A stand-in adapter is
//! used throughout; no test calls a provider. The mesh does not run in a test process, so
//! the lease step here is the local one; the claim's own refusal is the mesh's tests'.

mod common;

use std::sync::Arc;

use majordomus_cli::capability::builtin::implement::{implement, ImplementInput};
use majordomus_cli::capability::{CapabilityError, Context};
use majordomus_cli::execution::{
    Actor, ActorKind, EventPayload, ExecutionId, ExecutionStore, Limits, Progress, RepositoryRef,
    StepState,
};
use majordomus_cli::worker::{Adapter, Program};

/// The fixture's context, reporting into a fresh execution of `store`.
fn running(f: &common::Fixture) -> (Context, Arc<ExecutionStore>, ExecutionId) {
    let app = common::load_app(f);
    let store = Arc::new(ExecutionStore::new(Limits::default()));
    let id = ExecutionId::fresh();
    let cancel = store.create(
        id.clone(),
        "implement.issue",
        "Implement an issue",
        serde_json::json!({}),
        true,
        Actor::of(ActorKind::Internal),
        RepositoryRef {
            name: "fixture".into(),
            id: "id".into(),
            branch: None,
        },
    );
    store.publish(&id, EventPayload::Started);
    let mut ctx = (*app.context).clone();
    ctx.progress = Progress::reporting(Arc::clone(&store), id.clone(), cancel);
    (ctx, store, id)
}

fn input(issue: &str) -> ImplementInput {
    ImplementInput {
        issue: issue.into(),
        deadline_minutes: Some(1),
    }
}

fn worker(script: &str) -> Vec<Box<dyn Adapter>> {
    vec![Box::new(Program::new(
        "fixture",
        "sh",
        &["-c", script, "worker"],
    ))]
}

fn steps(store: &ExecutionStore, id: &ExecutionId) -> Vec<(String, StepState)> {
    store
        .get(id)
        .unwrap()
        .steps
        .iter()
        .map(|s| (s.name.clone(), s.state))
        .collect()
}

#[test]
fn outside_an_execution_it_is_refused() {
    let f = common::Fixture::new();
    let ctx = (*common::load_app(&f).context).clone();
    let refused = implement(&ctx, &input("I0001"), &worker("true")).unwrap_err();
    assert!(
        matches!(&refused, CapabilityError::Refused(m) if m.contains("only as an execution")),
        "{refused:?}"
    );
}

#[test]
fn a_ready_issue_is_worked_in_its_own_worktree_through_four_steps() {
    let f = common::Fixture::new();
    let (ctx, store, id) = running(&f);
    let report = implement(
        &ctx,
        &input("I0001"),
        &worker("echo working; echo done > made-by-worker"),
    )
    .expect("a ready issue is implemented");
    assert_eq!(report.branch, "feature/I0001-issue-I0001");
    assert_eq!(report.adapter, "fixture");
    let worktree = std::path::Path::new(&report.worktree);
    assert!(
        worktree.join("made-by-worker").exists(),
        "the worker ran in the issue's worktree, not the checkout"
    );
    assert!(!f.root().join("made-by-worker").exists());
    assert_eq!(
        steps(&store, &id),
        [
            ("readiness".into(), StepState::Completed),
            ("lease".into(), StepState::Completed),
            ("worktree".into(), StepState::Completed),
            ("dispatch".into(), StepState::Completed),
        ]
    );
    let logs: Vec<String> = store
        .events(&id, 0, 1000)
        .unwrap()
        .events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::Log { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect();
    assert!(logs.iter().any(|l| l == "working"), "{logs:?}");
}

#[test]
fn an_issue_that_is_not_ready_is_refused_before_anything_else() {
    let f = common::Fixture::new();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &common::ISSUE
            .replace("id: I0001", "id: I0002")
            .replace("slug: issue-I0001", "slug: issue-I0002")
            .replace("scope:\n", "depends_on:\n  - I0001\nscope:\n"),
    );
    f.commit("a dependent issue");
    let (ctx, store, id) = running(&f);
    let refused = implement(&ctx, &input("I0002"), &worker("true")).unwrap_err();
    assert!(
        matches!(&refused, CapabilityError::Refused(m) if m.starts_with("not_ready: I0002")),
        "{refused:?}"
    );
    assert_eq!(
        steps(&store, &id),
        [("readiness".into(), StepState::Failed)],
        "no step after readiness"
    );
}

#[test]
fn an_issue_the_plan_does_not_declare_is_not_found() {
    let f = common::Fixture::new();
    let (ctx, _, _) = running(&f);
    let refused = implement(&ctx, &input("I9999"), &worker("true")).unwrap_err();
    assert!(
        matches!(refused, CapabilityError::NotFound(_)),
        "{refused:?}"
    );
}

#[test]
fn with_no_worker_available_the_dispatch_is_refused_naming_each_reason() {
    let f = common::Fixture::new();
    let (ctx, store, id) = running(&f);
    let absent: Vec<Box<dyn Adapter>> = vec![Box::new(Program::new(
        "absent",
        "no-such-program-anywhere",
        &[],
    ))];
    let refused = implement(&ctx, &input("I0001"), &absent).unwrap_err();
    assert!(
        matches!(&refused, CapabilityError::Refused(m) if m.starts_with("no_worker: absent:")),
        "{refused:?}"
    );
    assert_eq!(
        steps(&store, &id).last().cloned(),
        Some(("dispatch".into(), StepState::Failed))
    );
}

#[test]
fn a_failing_worker_fails_the_implementation() {
    let f = common::Fixture::new();
    let (ctx, _, _) = running(&f);
    let failed = implement(&ctx, &input("I0001"), &worker("exit 4")).unwrap_err();
    assert!(
        matches!(&failed, CapabilityError::Internal(m) if m.contains("exited 4")),
        "{failed:?}"
    );
}
