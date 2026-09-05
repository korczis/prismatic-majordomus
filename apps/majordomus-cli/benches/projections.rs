//! Benchmarks of the hot paths: the YAML subset, front matter, glob matching, an index
//! built from a disposable repository, the registry composed over it, the OpenAPI document,
//! and one MCP `resources/list` answered. Run with `cargo bench`; numbers are reported, not
//! asserted, and no budget is promised in the documentation until one is measured on CI.

use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use majordomus_cli::capability::{builtin, CapabilityRegistry, Context};
use majordomus_cli::discovery::{FileSystem, Sources};
use majordomus_cli::git::GitState;
use majordomus_cli::http::openapi;
use majordomus_cli::mcp::{Server, Surface};
use majordomus_cli::metadata::{frontmatter, yaml, KindSchema};
use majordomus_cli::share::Share;
use majordomus_cli::{Index, Repository};
use std::sync::Arc;

const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  profiles: repo/profiles\n  rules: repo/rules\n  prompts: repo/prompts\n  skills: repo/skills\n  workflows: repo/workflows\n  knowledge: repo/knowledge\n  adrs: repo/adrs\n  project: repo/project\n";
const SOURCES: &str = "version: 1\nsources:\n  - id: policy\n    kind: policy\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/policy.yaml'\n    required: true\n  - id: profile\n    kind: profile\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/profiles/*.yaml'\n    required: true\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n";

fn rule(i: usize) -> String {
    format!("---\nid: project.rule-{i}\nversion: 1\nkind: rule\ntitle: Rule {i}\ndescription: Rule number {i}, in one sentence.\nstatement: Do the thing number {i}.\nstatus: active\nclass: advisory\ndepends_on: []\ntags: [bench]\n---\n\n# Rationale\n\nBecause {i}.\n")
}

/// A repository with `n` rules, on the filesystem only (no git, so the walk is measured).
fn repository(n: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let write = |rel: &str, content: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    };
    write(".ai/manifest.yaml", MANIFEST);
    write(
        ".ai/repo/policy.yaml",
        "version: 1\ncontext:\n  always_loaded_budget_lines: 150\n",
    );
    write(".ai/repo/profiles/implementation.yaml", "name: implementation\ndescription: d\ncapability: standard\neffort: medium\nverbosity: concise\npresentation: engineering\ncheckpoint_interval: 15m\n");
    write(".ai/repo/knowledge/sources.yaml", SOURCES);
    for i in 0..n {
        write(&format!(".ai/repo/rules/project/rule-{i}.v1.md"), &rule(i));
    }
    (dir, root)
}

fn build_index(root: &Path) -> Index {
    let repo = Repository::discover(root).unwrap();
    let sources = Sources::load(&repo).unwrap();
    let share = Share::locate(
        Some(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
        root,
    )
    .unwrap();
    let schema = KindSchema::load(&share, &repo).unwrap();
    let fs = FileSystem {
        excluded: vec![".git".into(), repo.local_path()],
    };
    Index::build(
        &repo,
        &sources,
        &schema,
        &fs,
        GitState::Unavailable {
            reason: "bench".into(),
        },
    )
    .unwrap()
}

fn context(root: &Path) -> Arc<Context> {
    let index = build_index(root);
    let registry = CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .with_index(&index)
        .build()
        .unwrap();
    Arc::new(Context::new(Arc::new(index), Arc::new(registry)))
}

fn benches(c: &mut Criterion) {
    let text = rule(7);
    c.bench_function("frontmatter split + yaml subset parse (one rule)", |b| {
        b.iter(|| {
            let split = frontmatter::split(&text).unwrap();
            yaml::parse_mapping(split.front.unwrap()).unwrap()
        })
    });
    let glob = majordomus_cli::discovery::glob::Glob::new(".ai/repo/rules/**/*.md");
    c.bench_function("glob match (one path)", |b| {
        b.iter(|| glob.matches(".ai/repo/rules/vendor/majordomus/rules/scope-integrity.v1.md"))
    });

    let (_dir, root) = repository(200);
    c.bench_function("index build, filesystem walk, 200 rules", |b| {
        b.iter(|| build_index(&root))
    });
    let index = build_index(&root);
    c.bench_function("registry build over 200 rules + builtins", |b| {
        b.iter(|| {
            CapabilityRegistry::builder()
                .with_builtin(builtin::all())
                .with_index(&index)
                .build()
                .unwrap()
        })
    });
    let ctx = context(&root);
    c.bench_function("openapi document", |b| {
        b.iter(|| openapi::document(&ctx.registry, "bench").unwrap())
    });
    c.bench_function("mcp resources/list over 200 rules", |b| {
        b.iter_batched(
            || Server::new(Surface::new(ctx.clone()), "bench"),
            |mut server| {
                server.handle(
                    serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }),
                )
            },
            BatchSize::SmallInput,
        )
    });
    c.bench_function("capabilities.list call", |b| {
        b.iter(|| {
            ctx.registry
                .call(&ctx, "capabilities.list", serde_json::json!({}))
                .unwrap()
        })
    });
}

criterion_group!(projections, benches);
criterion_main!(projections);
