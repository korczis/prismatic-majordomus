//! Every HTTP route, timed through the router with the capability's own benchmark cases:
//! the registry says which capabilities are on the wire, the input type says what to send,
//! and [`Request::bind`] turns each case into the request a client would make. A route
//! added tomorrow is timed tomorrow, and a route whose input type has no case does not
//! compile. Numbers are reported, not asserted; `tests/http_serve.rs` proves the same
//! cases answer over a real socket.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use majordomus_cli::capability::{builtin, CapabilityRegistry, CaseContext, Context};
use majordomus_cli::discovery::{FileSystem, Sources};
use majordomus_cli::git::GitState;
use majordomus_cli::http::{Request, Router};
use majordomus_cli::metadata::KindSchema;
use majordomus_cli::share::Share;
use majordomus_cli::{Index, Repository};

const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  profiles: repo/profiles\n  rules: repo/rules\n  prompts: repo/prompts\n  skills: repo/skills\n  workflows: repo/workflows\n  knowledge: repo/knowledge\n  adrs: repo/adrs\n  project: repo/project\n";
const SOURCES: &str = "version: 1\nsources:\n  - id: policy\n    kind: policy\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/policy.yaml'\n    required: true\n  - id: profile\n    kind: profile\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/profiles/*.yaml'\n    required: true\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n";

fn rule(i: usize) -> String {
    format!("---\nid: project.rule-{i}\nversion: 1\nkind: rule\ntitle: Rule {i}\ndescription: Rule number {i}, in one sentence.\nstatement: Do the thing number {i}.\nstatus: active\nclass: advisory\ndepends_on: []\ntags: [bench]\n---\n\n# Rationale\n\nBecause {i}.\n")
}

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

fn context(root: &Path) -> Arc<Context> {
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
    let index = Index::build(
        &repo,
        &sources,
        &schema,
        &fs,
        GitState::Unavailable {
            reason: "bench".into(),
        },
    )
    .unwrap();
    let registry = CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .with_index(&index)
        .build()
        .unwrap();
    Arc::new(Context::new(Arc::new(index), Arc::new(registry)))
}

fn benches(c: &mut Criterion) {
    let (_dir, root) = repository(200);
    let ctx = context(&root);
    let router = Router::new(ctx.clone(), "bench");
    let cases = CaseContext { index: &ctx.index };

    // the routes the registry declares, each with the cases its input type declares
    let mut group = c.benchmark_group("routes");
    for capability in ctx.registry.iter() {
        let Some(http) = &capability.exposure.http else {
            continue;
        };
        let Some(provider) = ctx.registry.cases(capability.id.as_str()) else {
            continue;
        };
        for case in provider(&cases) {
            let request = Request::bind(http.method, &http.path, &case.input);
            let label = format!(
                "{} {} [{}]",
                http.method.as_str(),
                request.target(),
                case.name
            );
            group.bench_function(label, |b| b.iter(|| router.handle(&request)));
        }
    }
    group.finish();

    // the projection's own routes, for the same table
    let mut group = c.benchmark_group("infrastructure");
    for target in ["/", "/openapi.json", "/docs"] {
        let request = Request::parse_target("GET", target, vec![]);
        let _ = router.handle(&request);
        group.bench_function(format!("GET {target}"), |b| {
            b.iter(|| router.handle(&request))
        });
    }
    group.finish();
}

criterion_group!(routes, benches);
criterion_main!(routes);
