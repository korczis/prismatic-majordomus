//! The one execution path: every transport's call goes through the executor, the cache
//! follows the capability's policy and nothing else, an answer from the cache is the
//! answer the handler gave, errors and commands are never cached, entries are bounded and
//! expire, and the key is the canonical id, the normalised input and the registry
//! fingerprint. Driven by the registry: whatever declares a process cache is tested.

mod common;

use std::sync::Arc;

use common::Fixture;
use majordomus_cli::capability::executor::canonical_json;
use majordomus_cli::capability::{
    BenchmarkCases, CachePolicy, CapabilityKind, CaseContext, Context,
};
use majordomus_cli::perf::COUNTERS;
use serde_json::{json, Value};

fn snapshot() -> majordomus_cli::perf::CounterSnapshot {
    COUNTERS.snapshot()
}

/// The counters are process-wide and the tests of this file read deltas, so they run one
/// at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// Every executable's cases, from its input type, against the fixture's index.
fn cases_of(ctx: &Context, id: &str) -> Vec<(String, Value)> {
    let provider = ctx
        .registry
        .cases(id)
        .expect("an executable has a case provider");
    provider(&CaseContext { index: &ctx.index })
        .into_iter()
        .map(|c| (c.name.to_string(), c.input))
        .collect()
}

#[test]
fn every_cached_capability_answers_the_same_from_the_handler_the_cold_cache_and_the_warm_cache() {
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let cached: Vec<String> = ctx
        .registry
        .iter()
        .filter(|c| c.cache.is_enabled())
        .map(|c| c.id.to_string())
        .collect();
    assert!(
        !cached.is_empty(),
        "the registry declares at least one process cache"
    );
    for id in &cached {
        let capability = ctx.registry.get(id).unwrap();
        assert_ne!(
            capability.kind,
            CapabilityKind::Command,
            "{id}: commands are never cached"
        );
        for (case, input) in cases_of(&ctx, id) {
            // uncached: a fresh context with the same registry has an empty cache, and the
            // handler runs
            let fresh = Arc::new(Context::new(ctx.index.clone(), ctx.registry.clone()));
            let before = snapshot();
            let uncached = fresh.execute(id, input.clone()).unwrap();
            let after = snapshot();
            assert_eq!(
                after.handler_invocations,
                before.handler_invocations + 1,
                "{id}/{case}: the handler ran"
            );
            assert_eq!(
                after.cache_misses,
                before.cache_misses + 1,
                "{id}/{case}: a miss"
            );
            // cold, then warm, in the shared executor
            ctx.executor.clear();
            let cold = ctx.execute(id, input.clone()).unwrap();
            let before = snapshot();
            let warm = ctx.execute(id, input.clone()).unwrap();
            let after = snapshot();
            assert_eq!(
                after.handler_invocations, before.handler_invocations,
                "{id}/{case}: a hit does not run the handler"
            );
            assert_eq!(
                after.cache_hits,
                before.cache_hits + 1,
                "{id}/{case}: counted as a hit"
            );
            assert_eq!(uncached, cold, "{id}/{case}: cold equals uncached");
            assert_eq!(cold, warm, "{id}/{case}: warm equals cold");
            // the same input with keys in another order is the same key
            if let Value::Object(m) = &input {
                let mut reversed = serde_json::Map::new();
                for (k, v) in m.iter().rev() {
                    reversed.insert(k.clone(), v.clone());
                }
                let before = snapshot();
                let again = ctx.execute(id, Value::Object(reversed)).unwrap();
                assert_eq!(
                    snapshot().cache_hits,
                    before.cache_hits + 1,
                    "{id}/{case}: key order does not matter"
                );
                assert_eq!(again, warm);
            }
        }
    }
}

#[test]
fn errors_are_not_cached_and_uncached_capabilities_always_run() {
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    // objects.search refuses a blank query; the refusal is computed each time
    let entries = ctx.executor.cached_entries();
    for _ in 0..3 {
        let before = snapshot();
        let err = ctx
            .execute("objects.search", json!({ "query": "  " }))
            .unwrap_err();
        assert!(err.to_string().contains("refused"), "{err}");
        assert_eq!(
            snapshot().handler_invocations,
            before.handler_invocations + 1
        );
    }
    assert_eq!(
        ctx.executor.cached_entries(),
        entries,
        "no error was stored"
    );
    // repository.info declares no cache: the handler runs every time
    assert!(!ctx
        .registry
        .get("repository.info")
        .unwrap()
        .cache
        .is_enabled());
    for _ in 0..3 {
        let before = snapshot();
        ctx.execute("repository.info", json!({})).unwrap();
        let after = snapshot();
        assert_eq!(after.handler_invocations, before.handler_invocations + 1);
        assert_eq!(after.cache_hits, before.cache_hits);
        assert_eq!(after.cache_misses, before.cache_misses);
    }
    // an unknown id is not found, through the executor like anything else
    let err = ctx.execute("nope.none", json!({})).unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}

#[test]
fn the_cache_is_bounded_per_capability_by_its_policy() {
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let CachePolicy::Process { max_entries, .. } =
        ctx.registry.get("objects.search").unwrap().cache
    else {
        panic!("objects.search declares a process cache")
    };
    ctx.executor.clear();
    let before = snapshot();
    for i in 0..(max_entries + 5) {
        ctx.execute("objects.search", json!({ "query": format!("word-{i}") }))
            .unwrap();
    }
    assert_eq!(
        ctx.executor.cached_entries(),
        max_entries,
        "never more than the policy allows"
    );
    assert_eq!(
        snapshot().cache_evictions,
        before.cache_evictions + 5,
        "the oldest five were evicted"
    );
    // the oldest is gone (a miss), the newest is still there (a hit)
    let before = snapshot();
    ctx.execute("objects.search", json!({ "query": "word-0" }))
        .unwrap();
    assert_eq!(snapshot().cache_misses, before.cache_misses + 1);
    let before = snapshot();
    ctx.execute(
        "objects.search",
        json!({ "query": format!("word-{}", max_entries + 4) }),
    )
    .unwrap();
    assert_eq!(snapshot().cache_hits, before.cache_hits + 1);
}

#[test]
fn the_registry_fingerprint_follows_the_repository_content_and_is_part_of_the_key() {
    let _serial = serial();
    let f = Fixture::new();
    let a = common::load_app(&f);
    let fp_a = a.registry().fingerprint().to_string();
    assert_eq!(fp_a.len(), 64, "a sha-256 hex digest");
    // the same repository again: the same fingerprint (stable across processes)
    let a2 = common::load_app(&f);
    assert_eq!(a2.registry().fingerprint(), fp_a);
    // one character of one rule changes: the index and the registry fingerprints move
    f.write(
        ".ai/repo/rules/project/alpha.v1.md",
        &common::rule("project.alpha", 1, "Alpha, edited"),
    );
    f.commit("edit");
    let b = common::load_app(&f);
    assert_ne!(b.registry().fingerprint(), fp_a);
    assert_ne!(b.index().fingerprint, a.index().fingerprint);
    // the key carries it: the two contexts never see each other's entries even through
    // one executor
    let shared = Arc::new(majordomus_cli::capability::CapabilityExecutor::new());
    let ctx_a = Context {
        executor: Arc::clone(&shared),
        ..(*a.context).clone()
    };
    let ctx_b = Context {
        executor: Arc::clone(&shared),
        ..(*b.context).clone()
    };
    let input = json!({ "query": "alpha" });
    let from_a = ctx_a.execute("objects.search", input.clone()).unwrap();
    let before = snapshot();
    let from_b = ctx_b.execute("objects.search", input.clone()).unwrap();
    assert_eq!(
        snapshot().cache_misses,
        before.cache_misses + 1,
        "another repository state is another key"
    );
    assert_ne!(from_a, from_b, "the edited title is in the answer");
    assert_eq!(shared.cached_entries(), 2);
}

#[test]
fn canonical_json_is_the_normal_form_of_an_input() {
    let _serial = serial();
    assert_eq!(
        canonical_json(&json!({"b": 1, "a": [{"y": 2, "x": 1}]})),
        r#"{"a":[{"x":1,"y":2}],"b":1}"#
    );
    assert_eq!(canonical_json(&json!(null)), "null");
    assert_eq!(canonical_json(&json!("s")), "\"s\"");
    // every builtin case is a JSON object; its normal form parses back to the same data
    let f = Fixture::new();
    let app = common::load_app(&f);
    for c in app.registry().iter().filter(|c| c.kind.is_executable()) {
        for (name, input) in cases_of(&app.context, c.id.as_str()) {
            let text = canonical_json(&input);
            let back: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(back, input, "{}/{name}", c.id);
            assert!(
                <majordomus_cli::capability::builtin::Empty as BenchmarkCases>::benchmark_cases(
                    &CaseContext {
                        index: &app.context.index
                    }
                )
                .len()
                    == 1
            );
        }
    }
}
