//! The benchmark model's parts in process: the policy read from data, the baseline files,
//! the statistics, the system targets, the result document, and the performance counters
//! and phases behind `perf.counters`.

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
