//! The benchmark projection is derived: every executable with a required policy is a
//! target directly and on every transport its exposure declares, with the cases its input
//! type provides; coverage's denominator is generated; adding or removing an exposure
//! adds or removes the requirement and the target with no other edit; the runners time
//! through the executor, a real socket and a real child; a baseline check reports every
//! line and never attaches a stale entry to a renamed target; and `majordomus bench`
//! answers all of it from the command line.

mod common;

use std::sync::Arc;

use common::{run_in, Fixture};
use majordomus_cli::bench::baseline::{Check, Policy};
use majordomus_cli::bench::results::{BenchmarkResult, CacheMode, Provenance, ResultDocument};
use majordomus_cli::bench::{
    BenchmarkProjection, Coverage, CoverageState, Profile, Runner, Statistics, SystemTarget,
    TargetKind, Transport, RESULT_SCHEMA,
};
use majordomus_cli::capability::handler::handler;
use majordomus_cli::capability::{
    builtin, BenchmarkCases, BenchmarkPolicy, CachePolicy, CanonicalSchema, Capability,
    CapabilityId, CapabilityKind, CapabilityRegistry, CaseContext, Context, Executable, Exposure,
    HttpExposure, HttpMethod, McpExposure, ModuleId, NamedCase, Provenance as Origin, Stability,
    WaiverReason,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Serialize, Deserialize, JsonSchema)]
struct EchoIn {
    text: String,
}
impl BenchmarkCases for EchoIn {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("short", EchoIn { text: "a".into() }),
            NamedCase::new(
                "long",
                EchoIn {
                    text: "a".repeat(64),
                },
            ),
        ]
    }
}
#[derive(Serialize, Deserialize, JsonSchema)]
struct NoCaseIn {
    text: String,
}
impl BenchmarkCases for NoCaseIn {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![]
    }
}
#[derive(Serialize, JsonSchema)]
struct EchoOut {
    text: String,
}

/// A fixture executable with the given exposures and policy.
fn fixture<I: BenchmarkCases + serde::de::DeserializeOwned + JsonSchema + 'static>(
    id: &str,
    mcp: bool,
    http: bool,
    benchmark: BenchmarkPolicy,
    cache: CachePolicy,
) -> Executable {
    Executable {
        capability: Capability {
            id: CapabilityId::parse(id).unwrap(),
            module: ModuleId::unchecked(""),
            kind: CapabilityKind::Query,
            title: format!("Fixture {id}"),
            description: "A fixture.".into(),
            input: CanonicalSchema::of::<I>(),
            output: CanonicalSchema::of::<EchoOut>(),
            provenance: Origin::Builtin {
                module: "fixture".into(),
            },
            exposure: Exposure {
                mcp: mcp.then(|| McpExposure {
                    tool: Some(id.replace('.', "_")),
                    resource: None,
                }),
                http: http.then(|| HttpExposure {
                    method: HttpMethod::Get,
                    path: format!("/api/v1/{}", id.replace('.', "-")),
                }),
                cli: None,
            },
            stability: Stability::Experimental,
            tags: vec![],
            benchmark,
            cache,
        },
        handler: handler::<Value, EchoOut, _>(|_, v| {
            Ok(EchoOut {
                text: v["text"].as_str().unwrap_or("").to_string(),
            })
        }),
        cases: <I as BenchmarkCases>::benchmark_cases_json,
    }
}

/// A context over the fixture repository's index and the given executables.
fn context(f: &Fixture, execs: Vec<Executable>) -> Arc<Context> {
    let app = common::load_app(f);
    let registry = CapabilityRegistry::builder()
        .with_builtin(execs)
        .with_index(&app.context.index)
        .build()
        .unwrap();
    Arc::new(Context::new(app.context.index.clone(), Arc::new(registry)))
}

#[test]
fn the_shipped_registry_is_fully_covered_and_every_target_traces_to_a_capability_or_a_system_operation(
) {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let projection = BenchmarkProjection::from_context(&ctx);
    let coverage = Coverage::compute(&ctx, &projection);
    assert!(coverage.is_complete(), "{}", coverage.render());
    let total = &coverage.tallies["total"];
    // the denominator is generated: every executable × its exposures, plus the system targets
    let expected: usize = ctx
        .registry
        .iter()
        .filter(|c| c.kind.is_executable())
        .map(|c| {
            1 + usize::from(c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()))
                + usize::from(c.exposure.http.is_some())
        })
        .sum::<usize>()
        + SystemTarget::ALL.len();
    assert_eq!(total.required, expected);
    assert_eq!(total.covered, expected);
    for t in &projection.targets {
        match &t.kind {
            TargetKind::Capability {
                id,
                transport,
                tool,
                route,
                ..
            } => {
                let c = ctx.registry.get(id).expect("a target names a capability");
                assert_eq!(c.benchmark, BenchmarkPolicy::Required);
                match transport {
                    Transport::Mcp => assert_eq!(
                        tool.as_deref(),
                        c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref())
                    ),
                    Transport::Http => assert_eq!(
                        route.as_ref().map(|(_, p)| p.as_str()),
                        c.exposure.http.as_ref().map(|h| h.path.as_str())
                    ),
                    Transport::Direct => assert!(tool.is_none() && route.is_none()),
                }
            }
            TargetKind::System { target } => assert!(SystemTarget::ALL.contains(target)),
        }
    }
    // every MCP tool and every HTTP route of the registry is a target
    for c in ctx.registry.iter().filter(|c| c.kind.is_executable()) {
        if c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()) {
            assert!(
                projection.covers(c.id.as_str(), Transport::Mcp),
                "{} over MCP",
                c.id
            );
        }
        if c.exposure.http.is_some() {
            assert!(
                projection.covers(c.id.as_str(), Transport::Http),
                "{} over HTTP",
                c.id
            );
        }
        assert!(
            projection.covers(c.id.as_str(), Transport::Direct),
            "{} directly",
            c.id
        );
    }
}

#[test]
fn exposure_and_policy_decide_the_targets_and_the_requirements_with_no_other_edit() {
    let f = Fixture::new();
    // mcp + http, two cases: 2 cases × 3 transports
    let ctx = context(
        &f,
        vec![fixture::<EchoIn>(
            "fixture.echo",
            true,
            true,
            BenchmarkPolicy::Required,
            CachePolicy::Disabled,
        )],
    );
    let p = BenchmarkProjection::from_context(&ctx);
    assert_eq!(p.of_capability("fixture.echo").count(), 6);
    let cov = Coverage::compute(&ctx, &p);
    let echo_lines: Vec<_> = cov
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.echo")
        .collect();
    assert_eq!(echo_lines.len(), 3);
    assert!(echo_lines
        .iter()
        .all(|l| l.state == CoverageState::Covered && l.cases == 2));

    // remove the HTTP exposure: the HTTP targets and the HTTP requirement disappear
    let ctx = context(
        &f,
        vec![fixture::<EchoIn>(
            "fixture.echo",
            true,
            false,
            BenchmarkPolicy::Required,
            CachePolicy::Disabled,
        )],
    );
    let p = BenchmarkProjection::from_context(&ctx);
    assert_eq!(p.of_capability("fixture.echo").count(), 4);
    assert!(!p.covers("fixture.echo", Transport::Http));
    let cov = Coverage::compute(&ctx, &p);
    assert!(cov
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.echo")
        .all(|l| l.transport != Transport::Http));
    assert!(cov.is_complete());

    // no case: required, exposed, and missing on every transport
    let ctx = context(
        &f,
        vec![fixture::<NoCaseIn>(
            "fixture.blank",
            true,
            true,
            BenchmarkPolicy::Required,
            CachePolicy::Disabled,
        )],
    );
    let p = BenchmarkProjection::from_context(&ctx);
    assert_eq!(p.of_capability("fixture.blank").count(), 0);
    let cov = Coverage::compute(&ctx, &p);
    let blank: Vec<_> = cov
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.blank")
        .collect();
    assert_eq!(blank.len(), 3);
    assert!(blank.iter().all(|l| l.state == CoverageState::Missing));
    assert!(!cov.is_complete() && !cov.has_no_missing());
    assert_eq!(cov.tallies["total"].missing, 3);
    assert!(cov.render().contains("MISSING  fixture.blank on direct"));

    // waived: reported, never covered, never a target
    let ctx = context(
        &f,
        vec![fixture::<EchoIn>(
            "fixture.waived",
            true,
            false,
            BenchmarkPolicy::Waived {
                reason: WaiverReason::Destructive,
            },
            CachePolicy::Disabled,
        )],
    );
    let p = BenchmarkProjection::from_context(&ctx);
    assert_eq!(p.of_capability("fixture.waived").count(), 0);
    let cov = Coverage::compute(&ctx, &p);
    let waived: Vec<_> = cov
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.waived")
        .collect();
    assert_eq!(waived.len(), 2);
    assert!(waived
        .iter()
        .all(|l| l.state == CoverageState::Waived && l.reason.as_deref() == Some("destructive")));
    assert!(!cov.is_complete() && cov.has_no_missing());
    assert_eq!(cov.tallies["total"].waived, 2);
}

#[test]
fn the_runners_time_a_target_through_the_executor_a_socket_and_a_child_process() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let projection = BenchmarkProjection::from_context(&ctx);
    let tiny = Profile {
        name: "test",
        warmup: 1,
        samples: 3,
        cold_spawns: 1,
    };
    let mut runner = Runner::new(ctx.clone(), tiny, &f.root())
        .with_executable(common::BIN.into())
        .with_share(common::dist_share());
    // one cached capability on every transport: cold and warm are both measured
    let search: Vec<_> = projection
        .of_capability("objects.search")
        .filter(|t| t.key.ends_with("|common-word"))
        .collect();
    assert_eq!(search.len(), 3);
    for t in search {
        let results = runner.run(t).unwrap();
        assert_eq!(results.len(), 2, "{}: cold and warm", t.key);
        assert_eq!(results[0].cache_mode, CacheMode::Cold);
        assert_eq!(results[1].cache_mode, CacheMode::Warm);
        for r in &results {
            assert_eq!(r.stats.samples, 3);
            assert!(
                r.stats.p50_us > 0.0 && r.stats.max_us >= r.stats.p50_us,
                "{}: {:?}",
                t.key,
                r.stats
            );
        }
        if t.transport() == Transport::Direct {
            assert_eq!(
                results[0].handler_invocations,
                Some(3),
                "cold: the handler ran every sample"
            );
            assert_eq!(results[1].handler_invocations, Some(0), "warm: never");
        }
    }
    // an uncached one, and the command (its direct run attaches the runner as a peer)
    let get = projection
        .of_capability("objects.get")
        .find(|t| t.transport() == Transport::Direct)
        .unwrap();
    let r = runner.run(get).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].cache_mode, CacheMode::Uncached);
    let announce = projection
        .of_capability("peers.announce")
        .find(|t| t.transport() == Transport::Direct)
        .unwrap();
    assert_eq!(
        runner.run(announce).unwrap()[0].handler_invocations,
        Some(3)
    );
    // every system target measures
    for s in SystemTarget::ALL {
        let t = projection
            .targets
            .iter()
            .find(|t| t.key == s.key())
            .unwrap();
        let r = runner.run(t).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].cache_mode, CacheMode::NotApplicable);
        assert!(r[0].stats.samples >= 1, "{}", s.key());
    }
    runner.finish();
}

fn doc(results: Vec<(&str, CacheMode, f64)>, fingerprint: &str) -> ResultDocument {
    ResultDocument {
        schema: RESULT_SCHEMA.into(),
        finished_at: "2026-01-01T00:00:00Z".into(),
        profile: "test".into(),
        provenance: Provenance {
            commit: Some("abc".into()),
            dirty: false,
            build_profile: "debug".into(),
            os: "testos".into(),
            arch: "testarch".into(),
            version: "0".into(),
            registry_fingerprint: fingerprint.into(),
        },
        results: results
            .into_iter()
            .map(|(key, mode, us)| BenchmarkResult {
                key: key.into(),
                kind: TargetKind::System {
                    target: SystemTarget::McpPing,
                },
                cache_mode: mode,
                stats: Statistics::of(&[Duration::from_nanos((us * 1000.0) as u64); 5]),
                handler_invocations: None,
            })
            .collect(),
    }
}

#[test]
fn a_check_reports_every_line_and_never_attaches_a_stale_baseline_to_a_renamed_target() {
    let policy = Policy::default();
    let base = doc(
        vec![
            ("a|direct|x", CacheMode::Uncached, 1000.0),
            ("old|direct|x", CacheMode::Uncached, 500.0),
            ("tiny", CacheMode::Uncached, 10.0),
        ],
        "fp1",
    );
    let run = doc(
        vec![
            ("a|direct|x", CacheMode::Uncached, 1400.0),
            ("new|direct|x", CacheMode::Uncached, 5.0),
            ("tiny", CacheMode::Uncached, 100.0),
        ],
        "fp2",
    );
    let check = Check::compare(&run, Some(&base), &policy);
    assert!(check.baseline_found);
    assert!(check.registry_changed);
    assert_eq!(check.new_targets, vec!["new|direct|x"]);
    assert_eq!(check.stale_baseline_targets, vec!["old|direct|x"]);
    let a_p50 = check
        .lines
        .iter()
        .find(|l| l.key == "a|direct|x" && l.metric == "p50")
        .unwrap();
    assert_eq!(
        a_p50.verdict, "FAIL",
        "+40% over a 25% allowance: {a_p50:?}"
    );
    assert!((a_p50.delta - 0.4).abs() < 1e-9);
    let tiny = check
        .lines
        .iter()
        .find(|l| l.key == "tiny" && l.metric == "p50")
        .unwrap();
    assert_eq!(
        tiny.verdict, "NOISE",
        "+900% but 90 µs: under the absolute floor"
    );
    assert!(check.failed());
    let text = check.render();
    assert!(
        text.contains("STALE  old|direct|x")
            && text.contains("NEW    new|direct|x")
            && text.contains("regression(s) found"),
        "{text}"
    );
    // within policy
    let run = doc(vec![("a|direct|x", CacheMode::Uncached, 1100.0)], "fp1");
    let check = Check::compare(&run, Some(&base), &policy);
    assert!(!check.failed() && !check.registry_changed);
    assert!(check.render().contains("within policy"));
    // no baseline: nothing compared, nothing failed
    let check = Check::compare(&run, None, &policy);
    assert!(!check.baseline_found && !check.failed());
    assert!(check
        .render()
        .contains("no baseline for testos-testarch-debug"));
}

#[test]
fn the_command_line_answers_coverage_runs_benchmarks_records_and_checks_a_baseline() {
    let f = Fixture::new();
    let (code, out, err) = run_in(&f.root(), &["bench", "coverage"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("Benchmark coverage")
            && out.contains("missing        0")
            && out.contains("waived         0"),
        "{out}"
    );
    let (code, out, _) = run_in(
        &f.root(),
        &["bench", "coverage", "--format", "json", "--check"],
        "",
    );
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema"], "majordomus/benchmark-coverage/v1");
    assert_eq!(v["tallies"]["total"]["missing"], 0);

    // no baseline yet: a check compares nothing and passes
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "bench",
            "objects.get",
            "--profile",
            "quick",
            "--transport",
            "direct",
            "--check",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let mut docs = out.split("}\n{");
    let run: Value = serde_json::from_str(&format!("{}}}", docs.next().unwrap())).unwrap();
    assert_eq!(run["schema"], RESULT_SCHEMA);
    assert_eq!(run["profile"], "quick");
    assert_eq!(run["results"].as_array().unwrap().len(), 1);
    assert_eq!(run["results"][0]["key"], "objects.get|direct|first-object");
    assert!(
        f.path(".ai/local/benchmarks").read_dir().unwrap().count() >= 1,
        "the run was written under the local half"
    );
    let check: Value = serde_json::from_str(&format!("{{{}", docs.next().unwrap())).unwrap();
    assert_eq!(check["baseline_found"], false);

    // a baseline from a dirty tree is refused, then recorded with --allow-dirty
    f.write("scratch.txt", "dirty\n");
    let (code, _, err) = run_in(
        &f.root(),
        &["bench", "baseline", "update", "--profile", "quick"],
        "",
    );
    assert_eq!(code, 13, "{err}");
    assert!(err.contains("dirty"), "{err}");
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "bench",
            "baseline",
            "update",
            "--profile",
            "quick",
            "--allow-dirty",
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains(".ai/repo/benchmarks/rust/baseline."), "{out}");
    let baseline = f
        .path(".ai/repo/benchmarks/rust")
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .find(|n| n.starts_with("baseline."))
        .expect("a baseline file");
    assert!(baseline.ends_with("-debug.json") || baseline.ends_with("-release.json"));
    // the same run again is within policy
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "bench",
            "objects.get",
            "--transport",
            "direct",
            "--profile",
            "quick",
            "--check",
            "--no-write",
        ],
        "",
    );
    assert_eq!(code, 0, "{err}\n{out}");
    assert!(out.contains("within policy"), "{out}");
    assert!(
        out.contains("STALE  system.mcp.ping"),
        "targets the baseline has and this filtered run did not measure are named: {out}"
    );
    // an unknown filter is not found; a bad flag is a usage error
    let (code, _, err) = run_in(&f.root(), &["bench", "nope.none", "--no-write"], "");
    assert_eq!(code, 12, "{err}");
    let (code, _, _) = run_in(&f.root(), &["bench", "--profile", "huge"], "");
    assert_eq!(code, 2);
    // the flat view still composes everything the modules do
    assert_eq!(
        builtin::all().len(),
        builtin::modules()
            .iter()
            .map(|m| m.capabilities.len())
            .sum::<usize>()
    );
}
