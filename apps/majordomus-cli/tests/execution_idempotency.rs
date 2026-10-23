//! A start under an idempotency key is idempotent (I3981, ADR 0225): the same key and input
//! answer the first execution, the same key with anything else is refused naming it, two
//! starts racing with one key yield one execution, and the key outlives a restart.

mod common;

use std::sync::Arc;

use majordomus_cli::capability::{
    BenchmarkCases, CapabilityError, CapabilityRegistry, CaseContext, Context, Exposure, NamedCase,
    Stability,
};
use majordomus_cli::execution::{
    Actor, ActorKind, ExecutionEngine, ExecutionJournal, Limits, SubmitError,
};
use majordomus_cli::{capability, module};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, JsonSchema)]
struct EchoInput {
    /// Anything.
    #[serde(default)]
    word: String,
}

impl BenchmarkCases for EchoInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "none",
            EchoInput {
                word: String::new(),
            },
        )]
    }
}

#[derive(Serialize, JsonSchema)]
struct Echoed {
    /// The word.
    word: String,
}

fn echo(_: &Context, input: EchoInput) -> Result<Echoed, CapabilityError> {
    Ok(Echoed { word: input.word })
}

fn context(f: &common::Fixture, engine: Option<ExecutionEngine>) -> Context {
    let module = module! {
        id: "fixture",
        title: "Fixture",
        description: "For the idempotency tests.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "fixture.echo", title: "Echo",
                description: "Answers the word it was given.",
                input: EchoInput, output: Echoed, stability: Stability::Experimental,
                exposure: Exposure::default(), tags: [],
                handler: echo,
            },
        ],
    };
    let registry = CapabilityRegistry::builder()
        .with_modules(vec![module])
        .build()
        .expect("the fixture registry builds");
    let app = common::load_app(f);
    let mut ctx = Context::new(app.context.index.clone(), Arc::new(registry));
    if let Some(engine) = engine {
        ctx.executions = Arc::new(engine);
    }
    ctx
}

fn actor() -> Actor {
    Actor::of(ActorKind::Http)
}

#[test]
fn the_same_key_and_input_answer_the_first_execution() {
    let f = common::Fixture::new();
    let ctx = context(&f, None);
    let first = ctx
        .executions
        .submit_keyed(
            &ctx,
            "fixture.echo",
            json!({ "word": "a" }),
            actor(),
            Some("click-1"),
        )
        .unwrap();
    let again = ctx
        .executions
        .submit_keyed(
            &ctx,
            "fixture.echo",
            json!({ "word": "a" }),
            actor(),
            Some("click-1"),
        )
        .unwrap();
    assert_eq!(first.id, again.id, "one execution");
    assert_eq!(ctx.executions.store().len(), 1);
    assert_eq!(again.idempotency_key.as_deref(), Some("click-1"));
}

#[test]
fn the_same_key_with_another_input_is_refused_naming_the_first() {
    let f = common::Fixture::new();
    let ctx = context(&f, None);
    let first = ctx
        .executions
        .submit_keyed(
            &ctx,
            "fixture.echo",
            json!({ "word": "a" }),
            actor(),
            Some("click-1"),
        )
        .unwrap();
    let refused = ctx
        .executions
        .submit_keyed(
            &ctx,
            "fixture.echo",
            json!({ "word": "b" }),
            actor(),
            Some("click-1"),
        )
        .unwrap_err();
    assert_eq!(refused.code(), "conflict");
    match refused {
        SubmitError::KeyConflict { existing, .. } => assert_eq!(existing, first.id),
        other => panic!("{other}"),
    }
    assert_eq!(
        ctx.executions.store().len(),
        1,
        "nothing refused was created"
    );
}

#[test]
fn a_key_that_is_not_one_is_refused_before_anything_is_created() {
    let f = common::Fixture::new();
    let ctx = context(&f, None);
    let refused = ctx
        .executions
        .submit_keyed(&ctx, "fixture.echo", json!({}), actor(), Some("../etc"))
        .unwrap_err();
    assert_eq!(refused.code(), "validation_error");
    assert!(ctx.executions.store().is_empty());
}

#[test]
fn two_starts_racing_with_one_key_yield_one_execution() {
    let f = common::Fixture::new();
    let ctx = Arc::new(context(&f, None));
    let starts: Vec<_> = (0..8)
        .map(|_| {
            let ctx = Arc::clone(&ctx);
            std::thread::spawn(move || {
                ctx.executions
                    .submit_keyed(
                        &ctx,
                        "fixture.echo",
                        json!({ "word": "a" }),
                        actor(),
                        Some("race"),
                    )
                    .unwrap()
                    .id
            })
        })
        .collect();
    let ids: std::collections::BTreeSet<_> =
        starts.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(ids.len(), 1, "every racer was answered the same execution");
    assert_eq!(ctx.executions.store().len(), 1);
}

#[test]
fn the_key_outlives_a_restart() {
    let f = common::Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let journaled =
        || ExecutionEngine::journaled(Limits::default(), ExecutionJournal::at(dir.path()).unwrap());
    let first = {
        let ctx = context(&f, Some(journaled()));
        ctx.executions
            .submit_keyed(
                &ctx,
                "fixture.echo",
                json!({ "word": "a" }),
                actor(),
                Some("click-1"),
            )
            .unwrap()
            .id
    };
    let ctx = context(&f, Some(journaled()));
    let again = ctx
        .executions
        .submit_keyed(
            &ctx,
            "fixture.echo",
            json!({ "word": "a" }),
            actor(),
            Some("click-1"),
        )
        .unwrap();
    assert_eq!(
        again.id, first,
        "the restarted server answers the execution the key started"
    );
}
