//! Benchmarks of the shared server's paths: the Swagger shell and the OpenAPI document
//! answered by the router, one MCP message answered over `/mcp` with its session lookup,
//! a stdio `tools/list` answered locally while the HTTP workers are bound and idle
//! against the same call with no server at all (the cost the companion adds to the
//! owner's session, expected to be none), and one message forwarded by a bridge over a
//! real loopback socket (what a second client pays per message). Numbers are reported,
//! not asserted.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use majordomus_cli::capability::{builtin, CapabilityRegistry, Context};
use majordomus_cli::discovery::{FileSystem, Sources};
use majordomus_cli::git::GitState;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::server;
use majordomus_cli::http::{Request, Router};
use majordomus_cli::mcp::bridge::Bridge;
use majordomus_cli::mcp::{Server, Surface};
use majordomus_cli::metadata::KindSchema;
use majordomus_cli::peers::Transport;
use majordomus_cli::share::Share;
use majordomus_cli::{Index, Repository};
use serde_json::json;

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
    let scope = majordomus_cli::scope::Scope::load(&share, &repo).unwrap();
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
        scope,
    )
    .unwrap();
    let registry = CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .with_index(&index)
        .build()
        .unwrap();
    Arc::new(Context::new(Arc::new(index), Arc::new(registry)))
}

fn tools_list() -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })
}

fn benches(c: &mut Criterion) {
    let (_dir, root) = repository(200);
    let ctx = context(&root);

    // the router alone: no socket
    let router = Router::new(ctx.clone(), "bench");
    c.bench_function("router GET /docs (swagger shell, cached)", |b| {
        b.iter(|| router.handle(&Request::parse_target("GET", "/docs", vec![])))
    });
    let _ = router.handle(&Request::parse_target("GET", "/openapi.json", vec![]));
    c.bench_function(
        "router GET /openapi.json (rendered once, served from memory)",
        |b| b.iter(|| router.handle(&Request::parse_target("GET", "/openapi.json", vec![]))),
    );

    // one MCP message through /mcp with its session lookup, no socket
    let endpoint = Arc::new(McpEndpoint::new(
        ctx.clone(),
        "bench",
        "http://127.0.0.1:0".into(),
    ));
    let router_mcp = Router::new(ctx.clone(), "bench").with_mcp(endpoint.clone());
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "bench", "version": "0" } } });
    let opened = router_mcp.handle(&Request::parse_target(
        "POST",
        "/mcp",
        init.to_string().into_bytes(),
    ));
    let session = opened
        .headers
        .iter()
        .find(|(k, _)| k == "Mcp-Session-Id")
        .map(|(_, v)| v.clone())
        .expect("a session");
    let body = tools_list().to_string().into_bytes();
    c.bench_function(
        "POST /mcp tools/list (session lookup + protocol), 200 rules",
        |b| {
            b.iter(|| {
                router_mcp.handle(
                    &Request::parse_target("POST", "/mcp", body.clone())
                        .with_headers(vec![("Mcp-Session-Id".into(), session.clone())]),
                )
            })
        },
    );

    // the owner's stdio session: the same call with nothing else running, and with the
    // shared server's workers bound and idle beside it
    let surface = Surface::new(ctx.clone());
    let peer = ctx.peers.attach(Transport::Stdio);
    c.bench_function("stdio tools/list, no shared server", |b| {
        b.iter_batched(
            || Server::new(surface.for_peer(peer.clone()), "bench"),
            |mut server| server.handle(tools_list()),
            BatchSize::SmallInput,
        )
    });
    let running = server::bind("127.0.0.1", 0)
        .unwrap()
        .start(router_mcp.clone());
    c.bench_function("stdio tools/list, shared server bound and idle", |b| {
        b.iter_batched(
            || Server::new(surface.for_peer(peer.clone()), "bench"),
            |mut server| server.handle(tools_list()),
            BatchSize::SmallInput,
        )
    });

    // what a second client pays: one message forwarded over a real loopback socket
    let mut bridge = Bridge::new(running.url());
    bridge.handle(&init).unwrap();
    let ping = json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" });
    c.bench_function("bridge ping over loopback (one request per message)", |b| {
        b.iter(|| bridge.handle(&ping).unwrap())
    });
    c.bench_function("bridge tools/list over loopback, 200 rules", |b| {
        b.iter(|| bridge.handle(&tools_list()).unwrap())
    });
    bridge.close();
    running.stop();
}

criterion_group!(shared, benches);
criterion_main!(shared);
