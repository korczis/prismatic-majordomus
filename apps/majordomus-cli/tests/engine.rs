//! The engine's own behaviour, against capabilities written for it: what it refuses, what
//! it does with a handler that panics, how a serial capability takes turns with itself,
//! what cancelling does at each point of a lifecycle, and what shutting down leaves behind.
//!
//! The fixture capabilities live here rather than in the crate: they exist to be waited on
//! and to panic, and neither belongs in a registry anybody ships.

mod common;

use std::sync::Arc;

use majordomus_cli::capability::{
    BenchmarkCases, CachePolicy, CapabilityError, CapabilityKind, CapabilityRegistry, CaseContext,
    Context, Exposure, NamedCase, Stability,
};
use majordomus_cli::execution::{
    Actor, ActorKind, Execution, ExecutionId, ExecutionState, SubmitError,
};
use majordomus_cli::{capability, module};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A capability that waits until it is told to stop, so the engine's own behaviour can
/// be seen: the queue, the serial policy, cancellation and shutdown.
#[derive(Serialize, Deserialize, JsonSchema)]
struct WaitInput {
    /// How many tenths of a second to wait for, at most.
    #[serde(default)]
    tenths: u64,
}

impl BenchmarkCases for WaitInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("none", WaitInput { tenths: 0 })]
    }
}

#[derive(Serialize, JsonSchema)]
struct Waited {
    /// How many tenths it actually waited.
    waited: u64,
}

fn wait(ctx: &Context, input: WaitInput) -> Result<Waited, CapabilityError> {
    let p = &ctx.progress;
    p.step("wait", "Waiting");
    for n in 0..input.tenths {
        if p.cancelled() {
            return Err(p.cancellation());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        p.progress(n + 1, Some(input.tenths), "waiting");
    }
    Ok(Waited {
        waited: input.tenths,
    })
}

fn boom(_: &Context, _: WaitInput) -> Result<Waited, CapabilityError> {
    panic!("a handler that does the worst thing it can do");
}

fn context(f: &common::Fixture) -> Context {
    let module = module! {
        id: "fixture",
        title: "Fixture",
        description: "For the engine's own tests.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "fixture.wait", title: "Wait",
                description: "Waits, reports, and stops when it is asked to.",
                input: WaitInput, output: Waited, stability: Stability::Experimental,
                exposure: Exposure::default(), tags: [],
                cache: CachePolicy::Disabled, handler: wait,
            }
            .cancellable(),
            capability! {
                id: "fixture.turn", kind: CapabilityKind::Command, title: "Take turns",
                description: "The same waiting, as a command: serial, because a command changes something.",
                input: WaitInput, output: Waited, stability: Stability::Experimental,
                exposure: Exposure::default(), tags: [],
                handler: wait,
            },
            capability! {
                id: "fixture.boom", title: "Panic",
                description: "Panics.",
                input: WaitInput, output: Waited, stability: Stability::Experimental,
                exposure: Exposure::default(), tags: [],
                handler: boom,
            },
        ],
    };
    let registry = CapabilityRegistry::builder()
        .with_modules(vec![module])
        .build()
        .expect("the fixture registry builds");
    let app = common::load_app(f);
    Context::new(app.context.index.clone(), Arc::new(registry))
}

fn settle(ctx: &Context, id: &ExecutionId, want: ExecutionState) -> Execution {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let snapshot = ctx.executions.store().get(id).expect("the execution");
        if snapshot.state == want || std::time::Instant::now() >= deadline {
            return snapshot;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn what_is_not_runnable_is_refused_before_anything_is_created() {
    let f = common::Fixture::new();
    let ctx = context(&f);
    let actor = Actor::of(ActorKind::Internal);
    let unknown = ctx
        .executions
        .submit(&ctx, "nothing.here", Value::Null, actor.clone())
        .unwrap_err();
    assert!(matches!(unknown, SubmitError::UnknownCapability(_)));
    assert_eq!(unknown.code(), "unknown_capability");

    let invalid = ctx
        .executions
        .submit(
            &ctx,
            "fixture.wait",
            serde_json::json!({ "tenths": "many" }),
            actor.clone(),
        )
        .unwrap_err();
    assert!(matches!(invalid, SubmitError::InvalidInput(_)), "{invalid}");
    assert_eq!(invalid.code(), "validation_error");
    assert!(invalid.to_string().contains("tenths"), "it names the field");

    assert!(
        ctx.executions.store().list(None, None, 10).is_empty(),
        "nothing refused was created"
    );
    assert!(
        majordomus_cli::execution::engine::runnable(
            CapabilityKind::Resource,
            Stability::Implemented
        )
        .is_some(),
        "a resource is read, never run, and the reason says so"
    );
}

#[test]
fn a_panicking_handler_fails_its_execution_and_nothing_else() {
    let f = common::Fixture::new();
    let ctx = context(&f);
    let boom = ctx
        .executions
        .submit(
            &ctx,
            "fixture.boom",
            Value::Null,
            Actor::of(ActorKind::Internal),
        )
        .expect("accepted");
    let failed = settle(&ctx, &boom.id, ExecutionState::Failed);
    assert_eq!(failed.state, ExecutionState::Failed);
    let error = failed.error.expect("an error");
    assert_eq!(error.code, "internal");
    assert_eq!(error.correlation_id, boom.id.to_string());
    assert!(
        !error.message.contains("worst thing"),
        "the panic's own words stay in this process's log: {}",
        error.message
    );

    // the engine is still working
    let after = ctx
        .executions
        .submit(
            &ctx,
            "fixture.wait",
            serde_json::json!({ "tenths": 0 }),
            Actor::of(ActorKind::Internal),
        )
        .expect("accepted");
    assert_eq!(
        settle(&ctx, &after.id, ExecutionState::Succeeded).state,
        ExecutionState::Succeeded
    );
}

#[test]
fn a_serial_capability_does_not_overlap_with_itself() {
    let f = common::Fixture::new();
    let ctx = context(&f);
    let actor = Actor::of(ActorKind::Internal);
    let a = ctx
        .executions
        .submit(
            &ctx,
            "fixture.turn",
            serde_json::json!({ "tenths": 3 }),
            actor.clone(),
        )
        .expect("accepted");
    let b = ctx
        .executions
        .submit(
            &ctx,
            "fixture.turn",
            serde_json::json!({ "tenths": 1 }),
            actor,
        )
        .expect("accepted");
    // a command is serial: the second waits, however many workers are free
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert_eq!(
        ctx.executions.store().get(&b.id).unwrap().state,
        ExecutionState::Queued,
        "the second waited for the first"
    );
    assert_eq!(
        ctx.executions.store().get(&a.id).unwrap().state,
        ExecutionState::Running
    );
    let first = settle(&ctx, &a.id, ExecutionState::Succeeded);
    let second = settle(&ctx, &b.id, ExecutionState::Succeeded);
    assert_eq!(first.state, ExecutionState::Succeeded);
    assert_eq!(second.state, ExecutionState::Succeeded);
    assert!(second.started_at >= first.finished_at, "{second:?}");

    // an unrestricted one does overlap, which is what makes the difference a policy
    let actor = Actor::of(ActorKind::Internal);
    let c = ctx
        .executions
        .submit(
            &ctx,
            "fixture.wait",
            serde_json::json!({ "tenths": 3 }),
            actor.clone(),
        )
        .expect("accepted");
    let d = ctx
        .executions
        .submit(
            &ctx,
            "fixture.wait",
            serde_json::json!({ "tenths": 3 }),
            actor,
        )
        .expect("accepted");
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert_eq!(
        ctx.executions.store().get(&c.id).unwrap().state,
        ExecutionState::Running
    );
    assert_eq!(
        ctx.executions.store().get(&d.id).unwrap().state,
        ExecutionState::Running,
        "two reads do not take turns"
    );
}

#[test]
fn cancelling_a_running_task_stops_it_and_cancelling_a_queued_one_never_starts_it() {
    let f = common::Fixture::new();
    let ctx = context(&f);
    let actor = Actor::of(ActorKind::Internal);
    let running = ctx
        .executions
        .submit(
            &ctx,
            "fixture.turn",
            serde_json::json!({ "tenths": 50 }),
            actor.clone(),
        )
        .expect("accepted");
    let queued = ctx
        .executions
        .submit(
            &ctx,
            "fixture.turn",
            serde_json::json!({ "tenths": 50 }),
            actor,
        )
        .expect("accepted");
    settle(&ctx, &running.id, ExecutionState::Running);

    assert_eq!(
        ctx.executions.cancel(&queued.id, "test"),
        majordomus_cli::execution::CancelOutcome::Requested
    );
    assert_eq!(
        ctx.executions.store().get(&queued.id).unwrap().state,
        ExecutionState::Cancelled,
        "a queued execution is cancelled without ever running"
    );

    ctx.executions.cancel(&running.id, "test");
    let stopped = settle(&ctx, &running.id, ExecutionState::Cancelled);
    assert_eq!(stopped.state, ExecutionState::Cancelled);
    assert!(
        stopped.duration_ms.is_some_and(|ms| ms < 5_000),
        "it stopped rather than running its five seconds out: {stopped:?}"
    );
}

#[test]
fn shutting_down_refuses_new_work_and_leaves_nothing_running() {
    let f = common::Fixture::new();
    let ctx = context(&f);
    let actor = Actor::of(ActorKind::Internal);
    let long = ctx
        .executions
        .submit(
            &ctx,
            "fixture.wait",
            serde_json::json!({ "tenths": 50 }),
            actor.clone(),
        )
        .expect("accepted");
    settle(&ctx, &long.id, ExecutionState::Running);
    assert!(ctx.executions.accepting());

    ctx.executions.shutdown(std::time::Duration::from_secs(5));
    assert!(!ctx.executions.accepting());
    let refused = ctx
        .executions
        .submit(&ctx, "fixture.wait", Value::Null, actor)
        .unwrap_err();
    assert!(matches!(refused, SubmitError::Unavailable(_)));
    assert_eq!(refused.code(), "unavailable");
    assert_eq!(
        ctx.executions.store().active(),
        0,
        "nothing is left in flight"
    );
    assert_eq!(
        ctx.executions.store().get(&long.id).unwrap().state,
        ExecutionState::Cancelled
    );
    assert_eq!(ctx.executions.queued(), 0);
    assert!(format!("{:?}", ctx.executions).starts_with("ExecutionEngine("));
}
