//! The Why catalogue's hot paths: building it from an index, and the queries every
//! projection runs over it — a lookup by id, a filter by audience, a full-text search, and
//! a diagnosis.
//!
//! Building is what happens once, when a context is composed. Everything else is what a
//! request does, and what a request does must not depend on the size of the catalogue in
//! the way a filesystem scan would. The catalogue is synthesised here at three sizes so the
//! shape of that dependence is visible rather than asserted.
//!
//! Numbers are reported, not asserted; no budget is promised in the documentation until one
//! is measured on CI.

use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use majordomus_cli::capability::{builtin, CapabilityRegistry};
use majordomus_cli::discovery::{FileSystem, Sources};
use majordomus_cli::git::GitState;
use majordomus_cli::metadata::KindSchema;
use majordomus_cli::share::Share;
use majordomus_cli::why::{Catalogue, Query};
use majordomus_cli::{Index, Repository};

const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  profiles: repo/profiles\n  rules: repo/rules\n  knowledge: repo/knowledge\n  why: repo/why\n";
const SOURCES: &str = "version: 1\nsources:\n  - id: policy\n    kind: policy\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/policy.yaml'\n    required: true\n  - id: profile\n    kind: profile\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/profiles/*.yaml'\n    required: true\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n  - id: moment\n    kind: moment\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/why/moments/*.md'\n    required: false\n  - id: audience\n    kind: audience\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/why/audiences/*.md'\n    required: false\n  - id: area\n    kind: area\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/why/areas/*.md'\n    required: false\n";

const AUDIENCES: usize = 8;
const AREAS: usize = 9;

fn audience(i: usize) -> String {
    format!("---\nschema: audience/v1\nid: audience-{i}\nkind: audience\ntitle: Audience {i}\nsummary: 'The {i}th audience of a synthetic catalogue.'\nstatus: stable\nweight: {i}\n---\n\n# Audience {i}\n\nBecause the benchmark says so.\n")
}

fn area(i: usize) -> String {
    format!("---\nschema: area/v1\nid: area-{i}\nkind: area\ntitle: Area {i}\nsummary: 'The {i}th area of a synthetic catalogue.'\nstatus: stable\nweight: {i}\n---\n\n# Area {i}\n\nBecause the benchmark says so.\n")
}

/// One moment, spread across the audiences and areas so that the reverse indexes are of a
/// realistic width rather than all pointing at one entry.
fn moment(i: usize) -> String {
    let a = i % AUDIENCES;
    let b = (i + 1) % AUDIENCES;
    let r = i % AREAS;
    let severity = ["low", "medium", "high"][i % 3];
    let related = if i > 0 {
        format!("related: [moment-{}]\n", i - 1)
    } else {
        String::new()
    };
    format!(
        "---\nschema: moment/v1\nid: moment-{i}\nkind: moment\ntitle: 'Moment {i}'\nhook: 'met the {i}th moment of a synthetic catalogue'\nsummary: 'The {i}th moment of a synthetic catalogue.'\nstatus: stable\nseverity: {severity}\nfrequency: common\nweight: {i}\naudiences: [audience-{a}, audience-{b}]\nareas: [area-{r}]\ntags: [bench, tag-{r}]\n{related}signals:\n  - id: signal-{i}\n    text: 'The {i}th symptom happened this week.'\nexamples:\n  - id: one\n    audience: audience-{a}\n    title: 'The first situation'\n    before: 'Nothing recorded it.'\n    after: 'The catalogue did.'\n  - id: two\n    audience: audience-{b}\n    title: 'The second situation'\n    before: 'Nothing recorded it.'\n    after: 'The catalogue did.'\n  - id: three\n    audience: audience-{a}\n    title: 'The third situation'\n    before: 'Nothing recorded it.'\n    after: 'The catalogue did.'\n---\n\n## The moment\n\nBecause the benchmark says so, {i} times.\n\n## Why it happens\n\nBecause the benchmark says so.\n\n## What it does not do\n\nNothing the benchmark does not say.\n"
    )
}

/// A repository whose why section holds `n` moments, on the filesystem only.
fn repository(n: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let write = |rel: String, content: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    };
    write(".ai/manifest.yaml".into(), MANIFEST);
    write(
        ".ai/repo/policy.yaml".into(),
        "version: 1\ncontext:\n  always_loaded_budget_lines: 150\n",
    );
    write(".ai/repo/profiles/implementation.yaml".into(), "name: implementation\ndescription: d\ncapability: standard\neffort: medium\nverbosity: concise\npresentation: engineering\ncheckpoint_interval: 15m\n");
    write(".ai/repo/knowledge/sources.yaml".into(), SOURCES);
    write(".ai/repo/rules/project/a.v1.md".into(), "---\nid: project.a\nversion: 1\nkind: rule\ntitle: A\ndescription: A.\nstatement: A.\nstatus: active\nclass: advisory\ndepends_on: []\ntags: []\n---\n\n# Rationale\n\nA.\n");
    for i in 0..AUDIENCES {
        write(
            format!(".ai/repo/why/audiences/audience-{i}.md"),
            &audience(i),
        );
    }
    for i in 0..AREAS {
        write(format!(".ai/repo/why/areas/area-{i}.md"), &area(i));
    }
    for i in 0..n {
        write(format!(".ai/repo/why/moments/moment-{i}.md"), &moment(i));
    }
    (dir, root)
}

fn index_of(root: &Path) -> Index {
    let repo = Repository::discover(root).unwrap();
    let share =
        Share::locate(Some(&majordomus_cli::synthetic::crate_share()), repo.root()).unwrap();
    let schema = KindSchema::load(&share, &repo).unwrap();
    let sources = Sources::load(&repo).unwrap();
    let scope = majordomus_cli::scope::Scope::load(&share, &repo).unwrap();
    Index::build(
        &repo,
        &sources,
        &schema,
        &FileSystem {
            excluded: vec![".git".into(), repo.local_path()],
        },
        GitState::Unavailable {
            reason: "bench".into(),
        },
        scope,
    )
    .unwrap()
}

fn registry_of(index: &Index) -> CapabilityRegistry {
    CapabilityRegistry::builder()
        .with_modules(builtin::modules())
        .with_index(index)
        .build()
        .unwrap()
}

fn build(c: &mut Criterion) {
    let mut g = c.benchmark_group("why/build");
    for n in [10usize, 100, 1000] {
        let (_dir, root) = repository(n);
        let index = index_of(&root);
        let registry = registry_of(&index);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| Catalogue::build(&index, &registry))
        });
    }
    g.finish();
}

fn queries(c: &mut Criterion) {
    let mut g = c.benchmark_group("why/query");
    for n in [10usize, 100, 1000] {
        let (_dir, root) = repository(n);
        let index = index_of(&root);
        let registry = registry_of(&index);
        let cat = Catalogue::build(&index, &registry);
        let last = format!("moment-{}", n - 1);
        // a lookup after the catalogue is built must not depend on its size
        g.bench_with_input(BenchmarkId::new("by-id", n), &n, |b, _| {
            b.iter(|| cat.moment(&last).is_some())
        });
        g.bench_with_input(BenchmarkId::new("by-audience", n), &n, |b, _| {
            b.iter(|| cat.naming("audience", "audience-3").len())
        });
        g.bench_with_input(BenchmarkId::new("filter", n), &n, |b, _| {
            b.iter(|| {
                cat.select(&Query {
                    audience: Some("audience-3".into()),
                    severity: Some("high".into()),
                    ..Query::default()
                })
                .len()
            })
        });
        g.bench_with_input(BenchmarkId::new("search", n), &n, |b, _| {
            b.iter(|| {
                cat.select(&Query {
                    q: Some("synthetic".into()),
                    ..Query::default()
                })
                .len()
            })
        });
        g.bench_with_input(BenchmarkId::new("facets", n), &n, |b, _| {
            b.iter(|| cat.facets())
        });
        g.bench_with_input(BenchmarkId::new("diagnose", n), &n, |b, _| {
            let picked: Vec<String> = (0..5).map(|i| format!("signal-{i}")).collect();
            b.iter(|| cat.diagnose(&picked))
        });
    }
    g.finish();
}

criterion_group!(benches, build, queries);
criterion_main!(benches);
