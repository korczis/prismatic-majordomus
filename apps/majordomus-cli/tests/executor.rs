//! The one execution path: every transport's call goes through the executor, the cache
//! follows the capability's policy and nothing else, an answer from the cache is the
//! answer the handler gave, errors and commands are never cached, entries are bounded and
//! expire, and the key is the canonical id, the normalised input and the registry
//! fingerprint. Driven by the registry: whatever declares a process cache is tested.

// claims: execution-cache-equivalence

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
    // plan.waves declares no cache: the handler runs every time
    assert!(!ctx.registry.get("plan.waves").unwrap().cache.is_enabled());
    for _ in 0..3 {
        let before = snapshot();
        ctx.execute("plan.waves", json!({})).unwrap();
        let after = snapshot();
        assert_eq!(after.handler_invocations, before.handler_invocations + 1);
        assert_eq!(after.cache_hits, before.cache_hits);
        assert_eq!(after.cache_misses, before.cache_misses);
    }
    // an unknown id is not found, through the executor like anything else
    let err = ctx.execute("nope.none", json!({})).unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}

/// A command changes state, so a descriptor that asks for a cache on one never reaches the
/// executor: the registry refuses it by id, and the same descriptor as a query builds.
#[test]
fn a_command_that_asks_for_a_cache_is_refused_by_the_registry() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::{
        Availability, BenchmarkPolicy, CanonicalSchema, Capability, CapabilityId,
        CapabilityRegistry, Executable, ExecutionPolicy, Exposure, ModuleId, Provenance,
        RegistryError, Stability, Visibility,
    };
    let _serial = serial();
    let cached = |kind: CapabilityKind| {
        let exposure = Exposure::default();
        Executable {
            capability: Capability {
                availability: Availability::classify(kind, &exposure),
                visibility: Visibility::classify(&exposure),
                id: CapabilityId::parse("fixture.write").unwrap(),
                module: ModuleId::unchecked(""),
                kind,
                title: "Fixture".into(),
                description: "A fixture capability.".into(),
                input: CanonicalSchema::empty(),
                output: CanonicalSchema::empty(),
                provenance: Provenance::Builtin {
                    module: "fixture".into(),
                },
                exposure,
                stability: Stability::Experimental,
                tags: vec![],
                benchmark: BenchmarkPolicy::Required,
                cache: CachePolicy::Process {
                    max_entries: 8,
                    ttl_seconds: None,
                },
                execution: ExecutionPolicy::classify(kind),
                skill: None,
            },
            handler: handler::<Value, Value, _>(|_, v| Ok(v)),
            cases: |_| vec![],
        }
    };
    let errs = CapabilityRegistry::builder()
        .with_builtin(vec![cached(CapabilityKind::Command)])
        .build()
        .expect_err("a cached command is refused");
    assert!(
        matches!(&errs[..], [RegistryError::InvalidCachePolicy { id, provenance, reason }]
            if id == "fixture.write" && provenance == "builtin fixture"
                && reason.contains("never cached")),
        "{errs:?}"
    );
    let registry = CapabilityRegistry::builder()
        .with_builtin(vec![cached(CapabilityKind::Query)])
        .build()
        .expect("the same descriptor as a query builds");
    assert!(registry.get("fixture.write").unwrap().cache.is_enabled());
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

/// A query of this file's own, with the cache it is given: what the tests below drive when
/// they need a handler they can count and slow down, or one that reads the work tree.
fn fixture_query(
    id: &str,
    cache: CachePolicy,
    handler: std::sync::Arc<dyn majordomus_cli::capability::Handler>,
) -> majordomus_cli::capability::Executable {
    use majordomus_cli::capability::{
        Availability, BenchmarkPolicy, CanonicalSchema, Capability, CapabilityId, Executable,
        ExecutionPolicy, Exposure, ModuleId, Provenance, Stability, Visibility,
    };
    let exposure = Exposure::default();
    let kind = CapabilityKind::Query;
    Executable {
        capability: Capability {
            availability: Availability::classify(kind, &exposure),
            visibility: Visibility::classify(&exposure),
            id: CapabilityId::parse(id).unwrap(),
            module: ModuleId::unchecked(""),
            kind,
            title: "Fixture".into(),
            description: "A fixture capability.".into(),
            input: CanonicalSchema::empty(),
            output: CanonicalSchema::empty(),
            provenance: Provenance::Builtin {
                module: "fixture".into(),
            },
            exposure,
            stability: Stability::Experimental,
            tags: vec![],
            benchmark: BenchmarkPolicy::Required,
            cache,
            execution: ExecutionPolicy::classify(kind),
            skill: None,
        },
        handler,
        cases: |_| vec![],
    }
}

/// A context over the fixture's index whose registry holds only `executables`.
fn context_with(
    app: &majordomus_cli::app::App,
    executables: Vec<majordomus_cli::capability::Executable>,
) -> Context {
    let registry = majordomus_cli::capability::CapabilityRegistry::builder()
        .with_builtin(executables)
        .build()
        .expect("the fixture registry builds");
    Context::new(app.context.index.clone(), Arc::new(registry))
}

/// Concurrent calls that miss on one key run the handler once: the first runs it, the
/// others wait for it and are answered with what it stored, which is what it returned.
#[test]
fn concurrent_misses_on_one_key_run_the_handler_once() {
    use majordomus_cli::capability::handler::handler;
    use std::sync::atomic::{AtomicU64, Ordering};
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let slow = handler::<Value, Value, _>(|_, v| {
        RAN.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(200));
        Ok(json!({ "echo": v, "n": RAN.load(Ordering::SeqCst) }))
    });
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.slow",
            CachePolicy::Process {
                max_entries: 4,
                ttl_seconds: None,
            },
            slow,
        )],
    );
    let barrier = std::sync::Barrier::new(8);
    let answers: Vec<Value> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    ctx.execute("fixture.slow", json!({ "k": 1 })).unwrap()
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });
    assert_eq!(
        RAN.load(Ordering::SeqCst),
        1,
        "one handler ran for eight callers"
    );
    assert_eq!(ctx.executor.handler_invocations(), 1);
    assert!(
        answers.iter().all(|a| a == &answers[0]),
        "every caller has the one answer: {answers:?}"
    );
    // another key is another flight, and runs its own handler
    ctx.execute("fixture.slow", json!({ "k": 2 })).unwrap();
    assert_eq!(ctx.executor.handler_invocations(), 2);
}

/// A failing handler stores nothing, and a caller that waited for it runs the handler
/// itself rather than being handed the failure or nothing.
#[test]
fn a_failure_is_not_shared_with_the_callers_that_waited_for_it() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::CapabilityError;
    use std::sync::atomic::{AtomicU64, Ordering};
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let first_fails = handler::<Value, Value, _>(|_, _| {
        let n = RAN.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(100));
        if n == 0 {
            Err(CapabilityError::Internal("the first call fails".into()))
        } else {
            Ok(json!({ "n": n }))
        }
    });
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.flaky",
            CachePolicy::Process {
                max_entries: 4,
                ttl_seconds: None,
            },
            first_fails,
        )],
    );
    let barrier = std::sync::Barrier::new(2);
    let answers: Vec<bool> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    ctx.execute("fixture.flaky", json!({})).is_ok()
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });
    assert_eq!(
        answers.iter().filter(|ok| **ok).count(),
        1,
        "one caller failed, the other ran the handler itself and succeeded: {answers:?}"
    );
    assert_eq!(RAN.load(Ordering::SeqCst), 2);
    assert_eq!(ctx.executor.cached_entries(), 1, "only the success is kept");
}

/// A `Generation` entry is kept for the context that computed it and its views, and a
/// new reading of the same repository — what a reload hands out — is a new key.
#[test]
fn a_generation_entry_is_kept_for_one_reading_of_the_repository() {
    use majordomus_cli::capability::handler::handler;
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let read = || {
        fixture_query(
            "fixture.reading",
            CachePolicy::Generation { max_entries: 2 },
            handler::<Value, Value, _>(|ctx, _| Ok(json!({ "objects": ctx.index.objects.len() }))),
        )
    };
    let first = context_with(&app, vec![read()]);
    let a = first.execute("fixture.reading", json!({})).unwrap();
    first
        .with_web(first.web.clone())
        .execute("fixture.reading", json!({}))
        .unwrap();
    assert_eq!(
        first.executor.handler_invocations(),
        1,
        "a view of the same reading answers from the entry"
    );
    // the same index and registry, read again, through the same executor
    let second = Context::new(first.index.clone(), first.registry.clone());
    assert_ne!(second.reading, first.reading);
    let second = Context {
        executor: Arc::clone(&first.executor),
        ..second
    };
    let b = second.execute("fixture.reading", json!({})).unwrap();
    assert_eq!(
        first.executor.handler_invocations(),
        2,
        "a new reading is a new key"
    );
    assert_eq!(a, b);
}

/// A `Checkout` entry is kept while the checkout is as it was, and every change the
/// fingerprint covers — an edit, a second edit of the same file, an untracked file, a
/// commit, a tag — is a new key whose answer is the one the handler gives.
#[test]
fn a_checkout_entry_follows_every_change_of_the_work_tree_and_the_refs() {
    use majordomus_cli::capability::handler::handler;
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.tree",
            CachePolicy::Checkout { max_entries: 64 },
            handler::<Value, Value, _>(|ctx, _| {
                let root = std::path::Path::new(&ctx.index.repository.root);
                let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
                let tags = std::process::Command::new("git")
                    .arg("-C")
                    .arg(root)
                    .args(["tag", "--list"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                    .unwrap_or_default();
                Ok(json!({
                    "readme": read("README.md"),
                    "new": read("NEW.md"),
                    "tags": tags,
                }))
            }),
        )],
    );
    let ask = |ctx: &Context| ctx.execute("fixture.tree", json!({})).unwrap();
    let fresh = |ctx: &Context| ctx.execute_observed("fixture.tree", json!({})).unwrap();
    let ran = |ctx: &Context| ctx.executor.handler_invocations();

    let first = ask(&ctx);
    assert_eq!(ask(&ctx), first);
    assert_eq!(ran(&ctx), 1, "nothing moved: the second call is a hit");

    let step = |what: &str, change: &dyn Fn()| {
        change();
        let before = ran(&ctx);
        let cached = ask(&ctx);
        assert_eq!(ran(&ctx), before + 1, "{what}: a new key, the handler ran");
        assert_eq!(cached, fresh(&ctx), "{what}: the answer is the handler's");
        assert_eq!(ask(&ctx), cached, "{what}: and then kept");
        cached
    };
    let edited = step("an edit of a tracked file", &|| {
        f.write("README.md", "# Fixture, edited\n")
    });
    assert_ne!(edited, first);
    std::thread::sleep(std::time::Duration::from_millis(20));
    let again = step("a second edit of the same file", &|| {
        f.write("README.md", "# Fixture, edited twice\n")
    });
    assert_ne!(again, edited);
    let untracked = step("an untracked file", &|| f.write("NEW.md", "new\n"));
    assert_ne!(untracked, again);
    step("a commit", &|| f.commit("change"));
    let tagged = step("a tag", &|| {
        f.git(&["tag", "v9.9.9"]);
    });
    assert_eq!(tagged["tags"], json!("v9.9.9\n"));
    // a file the fingerprint does not cover is ignored, and that is the policy's boundary
    f.write(".ai/local/state/elsewhere.yaml", "x: 1\n");
    let before = ran(&ctx);
    ask(&ctx);
    assert_eq!(ran(&ctx), before, "an ignored file is outside the key");
}

/// Every capability that keeps its answer per checkout state answers what its handler
/// answers after the state it reads has changed — a ledger row, a test, a commit — and
/// never what it kept before the change.
#[test]
fn every_checkout_capability_answers_fresh_after_the_state_it_reads_changed() {
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let checkout: Vec<String> = ctx
        .registry
        .iter()
        .filter(|c| matches!(c.cache, CachePolicy::Checkout { .. }))
        .map(|c| c.id.to_string())
        .collect();
    for want in [
        "evidence.report",
        "rules.report",
        "intents.list",
        "release.version",
    ] {
        assert!(
            checkout.iter().any(|id| id == want),
            "{want} keeps its answer per checkout state"
        );
    }
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let source = std::fs::read(f.path("test/cases/00_x.sh")).expect("the fixture's case");
    let ledger = json!({ "version": 1, "executions": [{
        "test": "suite:00_x", "runner": "suite", "source": "test/cases/00_x.sh",
        "outcome": "pass", "seconds": 1, "commit": head, "working_tree": "clean",
        "digest": majordomus_cli::evidence::digest_of(&source),
        "at": "2026-10-12T00:00:00Z", "origin": "local",
        "command": "bash test/run.sh 00_x"
    }]})
    .to_string();
    let changes: [(&str, &dyn Fn()); 3] = [
        ("a run recorded in the ledger", &|| {
            f.write(".ai/repo/evidence/ledger.json", &ledger)
        }),
        ("an edited case", &|| {
            f.write("test/cases/00_x.sh", "# majordomus-covers: none\ntrue\n")
        }),
        ("a commit", &|| f.commit("the state moved")),
    ];
    for id in &checkout {
        for (_, input) in cases_of(&ctx, id) {
            let _ = ctx.execute(id, input.clone());
        }
    }
    let report = |ctx: &Context| ctx.execute("evidence.report", json!({})).unwrap();
    let mut seen = vec![report(&ctx)];
    for (what, change) in changes {
        change();
        seen.push(report(&ctx));
        for id in &checkout {
            for (case, input) in cases_of(&ctx, id) {
                let fresh = ctx.execute_observed(id, input.clone());
                let cached = ctx.execute(id, input.clone());
                match (fresh, cached) {
                    (Ok(fresh), Ok(cached)) => {
                        assert_eq!(cached, fresh, "{id}/{case} after {what}")
                    }
                    (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
                    (a, b) => panic!("{id}/{case} after {what}: {a:?} against {b:?}"),
                }
            }
        }
    }
    // and the changes were ones the evidence report reads: each moved its answer
    for pair in seen.windows(2) {
        assert_ne!(pair[0], pair[1], "every change moved evidence.report");
    }
}

// ---- the latest reading of a source (capability::readings) ---------------------------

/// A second context over the same index, registry and executor with a reading of its own:
/// what a reload of the repository hands a long-lived process.
fn moved(first: &Context) -> Context {
    let second = Context::new(first.index.clone(), first.registry.clone());
    assert_ne!(second.reading, first.reading);
    Context {
        executor: Arc::clone(&first.executor),
        ..second
    }
}

/// Wait until `done` holds, for at most ten seconds: the bound of a worker of this file's
/// fixtures on a loaded machine, never a timing the test asserts.
fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done() {
        assert!(std::time::Instant::now() < until, "{what} did not happen");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// A source that has never been read and is slower than the caller's patience is handed
/// over as pending, at once, and its first reading arrives behind the caller: one handler
/// run for the caller that did not wait and the one that then did.
#[test]
fn a_first_reading_slower_than_the_patience_is_pending_and_arrives_behind_the_caller() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::readings::{read, Freshness, Patience, PENDING};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let slow = handler::<Value, Value, _>(|_, _| {
        std::thread::sleep(Duration::from_millis(1500));
        Ok(json!({ "n": RAN.fetch_add(1, Ordering::SeqCst) + 1 }))
    });
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.slow_first",
            CachePolicy::Disabled,
            slow,
        )],
    );
    let impatient = Patience {
        cold: Duration::from_millis(20),
        moved: Duration::from_millis(20),
    };
    let asked = Instant::now();
    let pending = read(&ctx, "fixture.slow_first", json!({}), impatient);
    assert!(
        asked.elapsed() < Duration::from_millis(1000),
        "the caller was handed something before the handler finished: {:?}",
        asked.elapsed()
    );
    assert_eq!(pending.freshness, Freshness::Pending);
    assert!(pending.refreshing, "its first reading is on its way");
    assert_eq!(pending.value, Err(PENDING.to_string()));
    assert_eq!(pending.age, None, "nothing was read, so nothing has an age");

    let arrived = read(&ctx, "fixture.slow_first", json!({}), Patience::WAIT);
    assert_eq!(arrived.freshness, Freshness::Current);
    assert_eq!(arrived.value, Ok(json!({ "n": 1 })));
    assert!(arrived.took >= Duration::from_millis(1500));
    assert_eq!(
        RAN.load(Ordering::SeqCst),
        1,
        "the caller that waited joined the reading the first one started"
    );
    assert_eq!(ctx.executor.readings_remembered(), 1);
}

/// A source that has been read is handed over at once from then on, however slow it is,
/// and is read again behind the caller once the reading is older than `REVALIDATE`.
#[test]
fn a_known_source_is_handed_over_at_once_and_read_again_behind_the_caller() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::readings::{read, Freshness, Patience, REVALIDATE};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let slow = handler::<Value, Value, _>(|_, _| {
        std::thread::sleep(Duration::from_millis(1500));
        Ok(json!({ "n": RAN.fetch_add(1, Ordering::SeqCst) + 1 }))
    });
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.slow_known",
            CachePolicy::Disabled,
            slow,
        )],
    );
    let first = read(&ctx, "fixture.slow_known", json!({}), Patience::WAIT);
    assert_eq!(first.value, Ok(json!({ "n": 1 })));

    std::thread::sleep(REVALIDATE + Duration::from_millis(100));
    let asked = Instant::now();
    let again = read(&ctx, "fixture.slow_known", json!({}), Patience::PAGE);
    assert!(
        asked.elapsed() < Duration::from_millis(1000),
        "a known source is not waited for: {:?}",
        asked.elapsed()
    );
    assert_eq!(again.value, Ok(json!({ "n": 1 })), "what is known now");
    assert_eq!(
        again.freshness,
        Freshness::Current,
        "the repository did not move"
    );
    assert!(again.age.is_some_and(|age| age >= REVALIDATE));
    assert!(again.refreshing, "it is being read again behind the caller");

    eventually("the second reading", || RAN.load(Ordering::SeqCst) == 2);
    eventually("the second reading landing", || {
        read(&ctx, "fixture.slow_known", json!({}), Patience::PAGE).value == Ok(json!({ "n": 2 }))
    });
    // a different input is a different source
    let other = read(
        &ctx,
        "fixture.slow_known",
        json!({ "k": 1 }),
        Patience::WAIT,
    );
    assert_eq!(other.value, Ok(json!({ "n": 3 })));
    assert_eq!(ctx.executor.readings_remembered(), 2);
}

/// When the repository has moved, a slow source is handed over as it was, marked stale,
/// within the caller's patience; a caller that waits gets the reading of the repository as
/// it is; and every other source read lately is read again without being asked for.
#[test]
fn a_reading_the_repository_moved_past_is_stale_at_once_and_everything_known_is_read_again() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::readings::{read, Freshness, Patience};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    static SLOW: AtomicU64 = AtomicU64::new(0);
    static OTHER: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let slow = handler::<Value, Value, _>(|_, _| {
        std::thread::sleep(Duration::from_millis(1500));
        Ok(json!({ "n": SLOW.fetch_add(1, Ordering::SeqCst) + 1 }))
    });
    let other = handler::<Value, Value, _>(|_, _| {
        Ok(json!({ "n": OTHER.fetch_add(1, Ordering::SeqCst) + 1 }))
    });
    let first = context_with(
        &app,
        vec![
            fixture_query("fixture.slow_moved", CachePolicy::Disabled, slow),
            fixture_query("fixture.other", CachePolicy::Disabled, other),
        ],
    );
    assert_eq!(
        read(&first, "fixture.slow_moved", json!({}), Patience::WAIT).value,
        Ok(json!({ "n": 1 }))
    );
    assert_eq!(
        read(&first, "fixture.other", json!({}), Patience::WAIT).value,
        Ok(json!({ "n": 1 }))
    );

    let second = moved(&first);
    let impatient = Patience {
        cold: Duration::from_millis(20),
        moved: Duration::from_millis(20),
    };
    let asked = Instant::now();
    let stale = read(&second, "fixture.slow_moved", json!({}), impatient);
    assert!(
        asked.elapsed() < Duration::from_millis(1000),
        "a moved repository is not waited for past the patience: {:?}",
        asked.elapsed()
    );
    assert_eq!(stale.freshness, Freshness::Stale);
    assert_eq!(stale.value, Ok(json!({ "n": 1 })), "as it was");
    assert!(stale.refreshing);
    assert!(stale.catching_up());

    // the other source was not asked for in the moved repository, and is read again
    eventually("the other source read again", || {
        OTHER.load(Ordering::SeqCst) == 2
    });

    let current = read(&second, "fixture.slow_moved", json!({}), Patience::WAIT);
    assert_eq!(current.freshness, Freshness::Current);
    assert_eq!(current.value, Ok(json!({ "n": 2 })));
    assert!(!current.catching_up());
    // a fast source is current within the patience of a page
    eventually("the other source current", || {
        let other = read(&second, "fixture.other", json!({}), Patience::PAGE);
        other.freshness == Freshness::Current && other.value == Ok(json!({ "n": 2 }))
    });
}

/// A source that cannot answer is a reading that says why, with an age like any other, and
/// it is read again; a handler that panics leaves a reading that says so, never a source
/// nobody reads any more.
#[test]
fn a_source_that_fails_or_panics_is_a_reading_that_says_so_and_is_read_again() {
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::readings::{read, Freshness, Patience, REVALIDATE};
    use majordomus_cli::capability::CapabilityError;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let troubled = handler::<Value, Value, _>(|_, _| match RAN.fetch_add(1, Ordering::SeqCst) {
        0 => Err(CapabilityError::Internal("the first reading fails".into())),
        1 => panic!("the second reading panics"),
        n => Ok(json!({ "n": n + 1 })),
    });
    let ctx = context_with(
        &app,
        vec![fixture_query(
            "fixture.troubled",
            CachePolicy::Disabled,
            troubled,
        )],
    );
    let failed = read(&ctx, "fixture.troubled", json!({}), Patience::WAIT);
    assert_eq!(
        failed.freshness,
        Freshness::Current,
        "a failure is a reading"
    );
    assert!(
        failed
            .value
            .as_ref()
            .is_err_and(|e| e.contains("the first reading fails")),
        "{:?}",
        failed.value
    );
    assert!(failed.age.is_some());

    std::thread::sleep(REVALIDATE + Duration::from_millis(100));
    read(&ctx, "fixture.troubled", json!({}), Patience::PAGE);
    eventually("the panic said as a reading", || {
        read(&ctx, "fixture.troubled", json!({}), Patience::PAGE)
            .value
            .is_err_and(|e| e.contains("panicked"))
    });
    std::thread::sleep(REVALIDATE + Duration::from_millis(100));
    eventually("the third reading", || {
        read(&ctx, "fixture.troubled", json!({}), Patience::PAGE).value == Ok(json!({ "n": 3 }))
    });
}

/// A change of the repository that sends many remembered sources to be read again starts
/// no more than `WORKERS` readers at once, and a reading a caller waits for is taken before
/// the ones nobody is waiting for.
#[test]
fn a_moved_repository_is_read_again_by_a_bounded_number_of_workers_and_a_waiting_caller_goes_first()
{
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::readings::{read, Freshness, Patience, WORKERS};
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::Duration;
    static RUNNING: AtomicUsize = AtomicUsize::new(0);
    static MOST: AtomicUsize = AtomicUsize::new(0);
    static RAN: AtomicU64 = AtomicU64::new(0);
    let _serial = serial();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let busy = handler::<Value, Value, _>(|_, v| {
        let now = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
        MOST.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(40));
        RUNNING.fetch_sub(1, Ordering::SeqCst);
        Ok(json!({ "k": v, "n": RAN.fetch_add(1, Ordering::SeqCst) }))
    });
    let quick = handler::<Value, Value, _>(|_, _| Ok(json!({ "quick": true })));
    let first = context_with(
        &app,
        vec![
            fixture_query("fixture.busy", CachePolicy::Disabled, busy),
            fixture_query("fixture.quick", CachePolicy::Disabled, quick),
        ],
    );
    const SOURCES: u64 = 40;
    for k in 0..SOURCES {
        read(&first, "fixture.busy", json!({ "k": k }), Patience::WAIT);
    }
    read(&first, "fixture.quick", json!({}), Patience::WAIT);
    MOST.store(0, Ordering::SeqCst);
    let ran = RAN.load(Ordering::SeqCst);

    // the repository moves: the busy sources go to the back of the queue, and the source a
    // caller then waits for is taken first
    let second = moved(&first);
    read(&second, "fixture.busy", json!({ "k": 0 }), Patience::PAGE);
    let quick = read(&second, "fixture.quick", json!({}), Patience::WAIT);
    assert_eq!(quick.freshness, Freshness::Current);
    let behind_it = RAN.load(Ordering::SeqCst) - ran;
    assert!(
        behind_it < SOURCES / 2,
        "the waiting caller went before most of the background ({behind_it} of {SOURCES} read first)"
    );

    eventually("every busy source read again", || {
        RAN.load(Ordering::SeqCst) - ran == SOURCES
    });
    let most = MOST.load(Ordering::SeqCst);
    assert!(
        (1..=WORKERS).contains(&most),
        "{most} readers at once, never more than {WORKERS}"
    );
}

/// What the cache holds is what the handler answered, handed to a caller as its own copy:
/// nothing before the capability was asked, and asking this way runs nothing.
#[test]
fn a_cached_answer_is_the_handlers_and_asking_for_it_runs_nothing() {
    let _serial = serial();
    let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
    let ctx = repo.context().unwrap();
    let describe = json!({ "id": "plan.status" });
    assert_eq!(
        ctx.executor
            .cached(&ctx, "capabilities.describe", &describe),
        None
    );
    assert_eq!(ctx.executor.handler_invocations(), 0);
    let answer = ctx
        .execute("capabilities.describe", describe.clone())
        .unwrap();
    let held = ctx
        .executor
        .cached(&ctx, "capabilities.describe", &describe);
    assert_eq!(held, Some(answer));
    assert_eq!(
        ctx.executor.handler_invocations(),
        1,
        "a lookup runs no handler"
    );
    assert_eq!(
        ctx.executor.cached(&ctx, "no.such.capability", &json!({})),
        None
    );
}
