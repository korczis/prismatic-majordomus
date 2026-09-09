//! Benchmarks of the command graph's hot paths: composing the graph, rendering the workflow
//! bridge, and answering a completion.
//!
//! The one that matters is the completion. It runs on every TAB, in front of a person who
//! did not choose to wait, and the whole design of the fast load — no index, no subprocess,
//! no network, no server — exists to keep it under the threshold at which a shell feels
//! slow. The others are here so that a change which moves the cost from the completion into
//! the composition is visible rather than invisible.
//!
//! The workflow contributor is deliberately absent from every case: it costs a subprocess,
//! which would measure `just` rather than this crate. What it contributes is measured where
//! it belongs, in `majordomus bench`, over the capability that serves the graph.
//!
//! Run with `cargo bench --bench commands`; numbers are reported, not asserted.

use criterion::{criterion_group, criterion_main, Criterion};
use majordomus_cli::capability::{builtin, CapabilityRegistry};
use majordomus_cli::command_graph::complete::{self, NoValues, Request};
use majordomus_cli::command_graph::{bridge, build, model::Surface, CommandGraph, Inputs};
use std::path::PathBuf;

/// The share directory of this repository, so the shell tool's registry is read too. The
/// crate sits two levels under the root.
fn share() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .to_path_buf();
    let share = root.join("share");
    share.join("commands.yaml").is_file().then_some(share)
}

fn registry() -> CapabilityRegistry {
    CapabilityRegistry::builder()
        .with_modules(builtin::modules())
        .build()
        .expect("the builtin registry composes")
}

fn graph_with(registry: Option<&CapabilityRegistry>, share: Option<&PathBuf>) -> CommandGraph {
    build(&Inputs {
        registry,
        share: share.map(|s| s.as_path()),
        workflows: None,
        bridged: Default::default(),
    })
}

fn compose(c: &mut Criterion) {
    let registry = registry();
    let share = share();
    let mut group = c.benchmark_group("command_graph/compose");
    // the command line alone: what a checkout with neither a shell tool nor a registry gets
    group.bench_function("clap-only", |b| {
        b.iter(|| std::hint::black_box(graph_with(None, None)))
    });
    // the join against the capability registry: MCP tool, HTTP route, schemas, effect
    group.bench_function("with-capabilities", |b| {
        b.iter(|| std::hint::black_box(graph_with(Some(&registry), None)))
    });
    // and the shell tool's own commands, read from the shipped registry
    group.bench_function("with-shell-tool", |b| {
        b.iter(|| std::hint::black_box(graph_with(Some(&registry), share.as_ref())))
    });
    group.finish();
}

fn project(c: &mut Criterion) {
    let registry = registry();
    let share = share();
    let graph = graph_with(Some(&registry), share.as_ref());
    let mut group = c.benchmark_group("command_graph/project");
    group.bench_function("bridge-render", |b| {
        b.iter(|| std::hint::black_box(bridge::render(&graph)))
    });
    group.bench_function("bridge-names", |b| {
        b.iter(|| std::hint::black_box(bridge::recipe_names(&graph)))
    });
    group.finish();
}

fn completion(c: &mut Criterion) {
    let registry = registry();
    let share = share();
    let graph = graph_with(Some(&registry), share.as_ref());

    let request = |surface: Surface, line: &str| {
        let words: Vec<String> = line.split_whitespace().map(String::from).collect();
        Request {
            surface,
            cursor: words.len(),
            words,
            cwd: None,
        }
    };

    let cases: Vec<(&str, Request)> = vec![
        ("cli-root", request(Surface::Cli, "majordomus")),
        ("cli-nested", request(Surface::Cli, "majordomus worktree")),
        (
            "cli-flags",
            request(Surface::Cli, "majordomus worktree status -"),
        ),
        (
            "cli-enum-value",
            request(Surface::Cli, "majordomus commands list --origin"),
        ),
        (
            "cli-dynamic-value",
            request(Surface::Cli, "majordomus worktree path"),
        ),
        ("workflow-root", request(Surface::Workflow, "just")),
        (
            "workflow-args",
            request(Surface::Workflow, "just worktree-status -"),
        ),
    ];

    let mut group = c.benchmark_group("command_graph/complete");
    for (name, request) in &cases {
        group.bench_function(*name, |b| {
            b.iter(|| std::hint::black_box(complete::complete(&graph, request, &NoValues)))
        });
    }
    group.finish();
}

criterion_group!(benches, compose, project, completion);
criterion_main!(benches);
