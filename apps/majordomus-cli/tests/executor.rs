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
            // The handler ran, and every execution the cache did not answer ran exactly
            // one handler. That identity — not `+ 1` — is the statement, because a handler
            // may *compose* another capability through the executor: `devcontext.compile`
            // reads `continuity.state` that way, so one call is two executions and two
            // handlers. The counter is right to count both. Counting only the outermost
            // would make composition invisible in the one instrument that shows it goes
            // through the executor at all rather than reaching into another module behind
            // it — and `executions - cache_hits` is already that number. What the counter
            // exists to catch is an execution that ran a handler twice, and that still
            // breaks this equality at any depth.
            let executions = after.executions - before.executions;
            let handlers = after.handler_invocations - before.handler_invocations;
            let hits = after.cache_hits - before.cache_hits;
            let misses = after.cache_misses - before.cache_misses;
            assert!(
                executions >= 1,
                "{id}/{case}: the call reached the executor"
            );
            assert_eq!(
                handlers,
                executions - hits,
                "{id}/{case}: the handler ran, once per execution the cache did not answer"
            );
            assert!(handlers >= 1, "{id}/{case}: the handler ran");
            // this capability is cached and its cache is empty, so at least one miss; a
            // composing call can add one per cached capability it reaches, never more than
            // the handlers that ran
            assert!(
                (1..=handlers).contains(&misses),
                "{id}/{case}: a miss ({misses} of {handlers} handler runs)"
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

/// A call answers from the distribution *this process* located, and not from one the
/// handler resolves for itself.
///
/// The witness is a stand-in distribution that nothing else could reach: the shipped one,
/// with an obligation vocabulary of a single token that exists in no file this repository
/// ships. The application is pointed at it, and what comes back has to be that token.
///
/// A handler that resolved its own would walk the conventions from the repository root —
/// which the fixture, being a repository and not a distribution, ships no `share/` in —
/// down to `<the executable>/../share`: a cargo target directory here, an installation
/// directory for a released binary. It would then answer with the shipped vocabulary when
/// the environment happened to name one and fail outright when it did not, so the answer
/// would be a function of where the binary lives and of who started it. That last step of
/// the ladder is what a released binary run inside a foreign repository needs, which is why
/// the process's distribution is handed to the handler rather than the ladder being changed.
#[test]
fn a_call_answers_from_the_distribution_the_process_located() {
    let _serial = serial();
    let f = Fixture::new();

    // the shipped distribution, with one file of it replaced
    let stand_in = tempfile::tempdir().expect("a directory for the stand-in distribution");
    for entry in std::fs::read_dir(common::dist_share()).expect("the shipped distribution") {
        let entry = entry.expect("an entry of the shipped distribution");
        if entry.file_name() == std::ffi::OsStr::new(VOCABULARY_FILE) {
            continue;
        }
        std::os::unix::fs::symlink(entry.path(), stand_in.path().join(entry.file_name()))
            .expect("the shipped file, linked into the stand-in");
    }
    std::fs::write(
        stand_in.path().join(VOCABULARY_FILE),
        "version: 1\nobligations:\n  - id: stand-in-only\n    title: The token only this distribution declares\n    summary: It exists in no file this repository ships.\n    discharged_by: none\n    remote: false\n",
    )
    .expect("the stand-in vocabulary");

    let app = majordomus_cli::app::App::load(&majordomus_cli::cli::RepoArgs {
        repo: Some(f.root()),
        share: Some(stand_in.path().to_path_buf()),
        ..Default::default()
    })
    .expect("the application loads over the stand-in distribution");
    let answer = app
        .context
        .execute("obligations.vocabulary", json!({}))
        .expect("the vocabulary is read");

    assert_eq!(
        answer["obligations"]
            .as_array()
            .map(|o| o.iter().map(|t| t["id"].clone()).collect::<Vec<_>>()),
        Some(vec![json!("stand-in-only")]),
        "the answer is the vocabulary of the distribution the process was given: {answer}"
    );
    assert_eq!(answer["count"], json!(1), "counted from the same file");
}

/// The vocabulary file of a distribution, named here so the test above can replace it
/// without the module under test exporting its own path.
const VOCABULARY_FILE: &str = "obligations.yaml";
