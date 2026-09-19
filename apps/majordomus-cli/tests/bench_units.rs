//! The benchmark model's parts in process: the policy read from data, the baseline files,
//! the statistics, the system targets, the result document, and the performance counters
//! and phases behind `perf.counters`.

// claims: benchmark-coverage-derived

mod common;

use std::time::Duration;

use common::Fixture;
use majordomus_cli::bench::baseline::{self, Policy};
use majordomus_cli::bench::results::{BenchmarkResult, CacheMode, Provenance, ResultDocument};
use majordomus_cli::bench::{
    BenchmarkProjection, Statistics, SystemTarget, TargetKind, Transport, RESULT_SCHEMA,
};
use majordomus_cli::capability::{builtin, CapabilityRegistry};
use majordomus_cli::perf::{self, Phase, COUNTERS};
use majordomus_cli::Repository;

#[test]
fn the_policy_is_data_with_a_default_and_a_file_that_overrides_it() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).unwrap();
    let default = Policy::load(&repo).unwrap();
    assert_eq!(default, Policy::default());
    assert_eq!(default.regression["p50"].relative, 0.25);
    assert!(baseline::policy_path(&repo).ends_with(".ai/repo/benchmarks/rust/policy.yaml"));
    f.write(
        ".ai/repo/benchmarks/rust/policy.yaml",
        "version: 1\nregression:\n  p95:\n    relative: 0.1\nminimum_absolute_us: 50\n",
    );
    let loaded = Policy::load(&repo).unwrap();
    assert_eq!(
        loaded.regression.len(),
        1,
        "the file replaces the default thresholds"
    );
    assert_eq!(loaded.regression["p95"].relative, 0.1);
    assert_eq!(loaded.minimum_absolute_us, 50.0);
    f.write(
        ".ai/repo/benchmarks/rust/policy.yaml",
        "version: 1\nregression:\n  p95:\n    relative: soon\n",
    );
    let err = Policy::load(&repo).unwrap_err();
    assert!(err.to_string().contains("regression.p95.relative"), "{err}");
    assert_eq!(err.exit_code(), 10);
}

fn document(fingerprint: &str) -> ResultDocument {
    ResultDocument {
        schema: RESULT_SCHEMA.into(),
        finished_at: "2026-01-02T03:04:05Z".into(),
        profile: "test".into(),
        provenance: Provenance {
            commit: None,
            dirty: true,
            build_profile: "debug".into(),
            os: "testos".into(),
            arch: "testarch".into(),
            version: "0".into(),
            registry_fingerprint: fingerprint.into(),
            host: "testhost/1".into(),
        },
        results: vec![BenchmarkResult {
            key: "system.mcp.ping".into(),
            kind: TargetKind::System {
                target: SystemTarget::McpPing,
            },
            cache_mode: CacheMode::NotApplicable,
            stats: Statistics::of(&[Duration::from_micros(7); 3]),
            handler_invocations: None,
        }],
    }
}

#[test]
fn baselines_are_written_and_read_per_platform_and_results_are_written_under_the_local_half() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).unwrap();
    assert!(
        !Provenance::of(&repo, "fp").dirty,
        "a committed fixture is clean"
    );
    assert!(baseline::load_baseline(&repo, "testos-testarch-debug")
        .unwrap()
        .is_none());
    let doc = document("fp");
    let path = baseline::write_baseline(&repo, &doc).unwrap();
    assert!(
        path.ends_with(".ai/repo/benchmarks/rust/baseline.testos-testarch-debug.json"),
        "{}",
        path.display()
    );
    let back = baseline::load_baseline(&repo, "testos-testarch-debug")
        .unwrap()
        .unwrap();
    assert_eq!(back, doc);
    assert_eq!(
        back.find("system.mcp.ping", CacheMode::NotApplicable)
            .unwrap()
            .stats
            .samples,
        3
    );
    assert!(back.find("system.mcp.ping", CacheMode::Warm).is_none());
    assert!(
        baseline::load_baseline(&repo, "other-platform-release")
            .unwrap()
            .is_none(),
        "another platform is not compared"
    );
    f.write(
        ".ai/repo/benchmarks/rust/baseline.testos-testarch-debug.json",
        "not json",
    );
    let err = baseline::load_baseline(&repo, "testos-testarch-debug").unwrap_err();
    assert!(
        err.to_string().contains("not a benchmark result document"),
        "{err}"
    );
    let local = doc.write_local(&repo).unwrap();
    assert!(
        local.starts_with(f.root().join(".ai/local/benchmarks")),
        "{}",
        local.display()
    );
    assert!(local
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("-test.json"));
    assert!(doc.render().ends_with('\n'));
    let real = Provenance::of(&repo, "fp");
    assert_eq!(
        real.build_profile,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    assert!(
        real.dirty,
        "the baseline and the local run are untracked files: the tree is dirty now"
    );
    assert!(real.commit.is_some());
    assert_eq!(
        real.platform(),
        format!(
            "{}-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            real.build_profile
        )
    );
}

#[test]
fn statistics_reduce_samples_and_answer_metrics_by_name() {
    let empty = Statistics::of(&[]);
    assert_eq!(empty.samples, 0);
    assert_eq!(empty.p95_us, 0.0);
    let one = Statistics::of(&[Duration::from_micros(42)]);
    assert_eq!(
        (one.min_us, one.p50_us, one.p99_us, one.max_us),
        (42.0, 42.0, 42.0, 42.0)
    );
    assert_eq!(one.stddev_us, 0.0);
    let s = Statistics::of(&(1..=10).map(Duration::from_micros).collect::<Vec<_>>());
    assert_eq!(s.p50_us, 5.0);
    assert_eq!(s.p90_us, 9.0);
    assert_eq!(s.p95_us, 10.0);
    for (name, value) in [
        ("min", s.min_us),
        ("p50", s.p50_us),
        ("p90", s.p90_us),
        ("p95", s.p95_us),
        ("p99", s.p99_us),
        ("max", s.max_us),
        ("mean", s.mean_us),
    ] {
        assert_eq!(s.metric(name), Some(value), "{name}");
    }
    assert_eq!(s.metric("p75"), None);
}

#[test]
fn system_targets_and_transports_are_named_once() {
    for s in SystemTarget::ALL {
        assert!(
            s.key()
                .starts_with(&format!("system.{}.", s.transport().name())),
            "{}",
            s.key()
        );
        assert!(!s.description().is_empty());
    }
    for t in Transport::ALL {
        assert_eq!(Transport::parse(t.name()), Some(t));
    }
    assert_eq!(Transport::parse("carrier-pigeon"), None);
    let f = Fixture::new();
    let app = common::load_app(&f);
    let projection = BenchmarkProjection::from_context(&app.context);
    let direct = projection.by_transport(Transport::Direct).count();
    let mcp = projection.by_transport(Transport::Mcp).count();
    let http = projection.by_transport(Transport::Http).count();
    assert_eq!(direct + mcp + http, projection.targets.len());
    assert!(BenchmarkProjection::is_command(
        &app.context.registry,
        "peers.announce"
    ));
    assert!(!BenchmarkProjection::is_command(
        &app.context.registry,
        "objects.get"
    ));
    let registry = CapabilityRegistry::builder()
        .with_modules(builtin::modules())
        .build()
        .unwrap();
    assert!(format!("{:?}", builtin::modules()[0]).contains("ModuleDescriptor"));
    assert!(registry.cases("rule.nope").is_none());
}

#[test]
fn counters_and_phases_are_readable_and_the_startup_set_is_named() {
    let before = COUNTERS.snapshot();
    {
        let _a = perf::phase(Phase::HandlerExecution);
        let _b = perf::phase(Phase::OpenApiBuild);
    }
    let after = COUNTERS.snapshot();
    assert!(after.phases["handler_execution"].count > before.phases["handler_execution"].count);
    assert!(after.phases["open_api_build"].count > before.phases["open_api_build"].count);
    for p in Phase::ALL {
        assert!(after.phases.contains_key(p.name()), "{}", p.name());
        assert_eq!(serde_json::to_value(p).unwrap(), p.name());
    }
    let names: Vec<&str> = after.startup_work().iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        [
            "repository_scans",
            "index_builds",
            "registry_builds",
            "schema_generations",
            "mcp_projection_builds",
            "openapi_builds",
            "http_projection_builds"
        ]
    );
    let json = serde_json::to_value(&after).unwrap();
    assert!(json["executions"].is_u64());
    let back: perf::CounterSnapshot = serde_json::from_value(json).unwrap();
    assert_eq!(back, after);
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct CasedIn {
    text: String,
}
impl majordomus_cli::capability::BenchmarkCases for CasedIn {
    fn benchmark_cases(
        _: &majordomus_cli::capability::CaseContext<'_>,
    ) -> Vec<majordomus_cli::capability::NamedCase<Self>> {
        vec![majordomus_cli::capability::NamedCase::new(
            "one",
            CasedIn { text: "a".into() },
        )]
    }
}
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct CaselessIn {
    text: String,
}
impl majordomus_cli::capability::BenchmarkCases for CaselessIn {
    fn benchmark_cases(
        _: &majordomus_cli::capability::CaseContext<'_>,
    ) -> Vec<majordomus_cli::capability::NamedCase<Self>> {
        vec![]
    }
}

/// A required query exposed on MCP and HTTP whose cases come from `I`.
fn exposed<I>(id: &str) -> majordomus_cli::capability::Executable
where
    I: majordomus_cli::capability::BenchmarkCases
        + serde::de::DeserializeOwned
        + schemars::JsonSchema
        + 'static,
{
    use majordomus_cli::capability::handler::handler;
    use majordomus_cli::capability::{
        Availability, BenchmarkCases, BenchmarkPolicy, CachePolicy, CanonicalSchema, Capability,
        CapabilityId, CapabilityKind, Executable, ExecutionPolicy, Exposure, HttpExposure,
        HttpMethod, McpExposure, ModuleId, Stability, Visibility,
    };
    let kind = CapabilityKind::Query;
    let exposure = Exposure {
        mcp: Some(McpExposure {
            tool: Some(id.replace('.', "_")),
            resource: None,
        }),
        http: Some(HttpExposure {
            method: HttpMethod::Get,
            path: format!("/api/v1/{}", id.replace('.', "-")),
        }),
        cli: None,
    };
    Executable {
        capability: Capability {
            availability: Availability::classify(kind, &exposure),
            visibility: Visibility::classify(&exposure),
            id: CapabilityId::parse(id).unwrap(),
            module: ModuleId::unchecked(""),
            kind,
            title: format!("Fixture {id}"),
            description: "A fixture.".into(),
            input: CanonicalSchema::of::<I>(),
            output: CanonicalSchema::empty(),
            provenance: majordomus_cli::capability::Provenance::Builtin {
                module: "fixture".into(),
            },
            exposure,
            stability: Stability::Experimental,
            tags: vec![],
            benchmark: BenchmarkPolicy::Required,
            cache: CachePolicy::Disabled,
            execution: ExecutionPolicy::classify(kind),
        },
        handler: handler::<serde_json::Value, serde_json::Value, _>(|_, v| Ok(v)),
        cases: <I as BenchmarkCases>::benchmark_cases_json,
    }
}

/// The denominator is generated from the registry — every executable directly and on each
/// transport it is exposed on, plus the system targets declared once — every required one
/// is a target exactly there, and a required executable whose input yields no case is
/// missing on every transport, which is the verdict `bench coverage --check` and
/// `capabilities validate` fail on.
#[test]
fn the_denominator_is_generated_from_the_registry_and_a_missing_case_fails_the_check() {
    use majordomus_cli::bench::{Coverage, CoverageState};
    use majordomus_cli::capability::{BenchmarkPolicy, Capability, Context};
    use std::sync::Arc;

    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let projection = BenchmarkProjection::from_context(&ctx);
    let coverage = Coverage::compute(&ctx, &projection);
    let exposures = |c: &Capability| {
        1 + usize::from(c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()))
            + usize::from(c.exposure.http.is_some())
    };
    let executable = || {
        ctx.registry
            .iter()
            .filter(|c| c.kind.is_executable() && c.stability.executable())
    };
    let total = &coverage.tallies["total"];
    assert_eq!(
        total.required,
        executable().map(exposures).sum::<usize>() + SystemTarget::ALL.len(),
        "the denominator is every exposure of every executable plus the system targets"
    );
    assert_eq!(total.missing, 0, "{}", coverage.render());
    assert!(coverage.has_no_missing());
    for c in executable().filter(|c| matches!(c.benchmark, BenchmarkPolicy::Required)) {
        for t in Transport::ALL {
            let exposed = match t {
                Transport::Direct => true,
                Transport::Mcp => c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()),
                Transport::Http => c.exposure.http.is_some(),
            };
            assert_eq!(
                projection.covers(c.id.as_str(), t),
                exposed,
                "{} on {}: a target exactly where it is exposed",
                c.id,
                t.name()
            );
        }
    }
    for s in SystemTarget::ALL {
        assert!(
            projection
                .targets
                .iter()
                .any(|t| matches!(&t.kind, TargetKind::System { target } if *target == s)),
            "{} is a target",
            s.key()
        );
    }

    // one more capability in the registry: the denominator follows with no other edit,
    // covered when its input yields a case and missing on every transport when it does not
    let with = |e| {
        let registry = CapabilityRegistry::builder()
            .with_builtin(vec![e])
            .with_index(&ctx.index)
            .build()
            .unwrap();
        let ctx = Arc::new(Context::new(ctx.index.clone(), Arc::new(registry)));
        let p = BenchmarkProjection::from_context(&ctx);
        Coverage::compute(&ctx, &p)
    };
    let covered = with(exposed::<CasedIn>("fixture.cased"));
    let lines: Vec<_> = covered
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.cased")
        .collect();
    assert_eq!(lines.len(), 3, "required on all three transports");
    assert!(lines.iter().all(|l| l.state == CoverageState::Covered));
    assert!(covered.has_no_missing(), "{}", covered.render());

    let missing = with(exposed::<CaselessIn>("fixture.caseless"));
    let lines: Vec<_> = missing
        .lines
        .iter()
        .filter(|l| l.subject == "fixture.caseless")
        .collect();
    assert_eq!(lines.len(), 3, "still required on all three transports");
    assert!(lines.iter().all(|l| l.state == CoverageState::Missing));
    assert_eq!(missing.tallies["total"].missing, 3);
    assert!(
        !missing.has_no_missing(),
        "a missing case fails the check\n{}",
        missing.render()
    );
    assert!(missing
        .render()
        .contains("MISSING  fixture.caseless on direct"));
}
