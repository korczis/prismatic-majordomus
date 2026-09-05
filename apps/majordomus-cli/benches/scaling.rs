//! Scaling: the phases that grow with the repository, measured over synthetic
//! repositories of several sizes so that a pathological order shows as a curve, not as a
//! feeling. Discovery and reading, index build, registry build, the OpenAPI document,
//! the MCP listings and one lookup, at 10, 100 and 1000 objects. Numbers are reported,
//! not asserted.

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use majordomus_cli::bench::BenchmarkProjection;
use majordomus_cli::capability::{builtin, CapabilityRegistry, Context};
use majordomus_cli::http::openapi;
use majordomus_cli::mcp::Surface;
use majordomus_cli::synthetic::{Shape, SyntheticRepository};

fn benches(c: &mut Criterion) {
    for rules in [10usize, 100, 1000] {
        let repo = SyntheticRepository::new(Shape {
            rules,
            prompts: 2,
            documents: rules / 10,
            body_lines: 8,
        })
        .unwrap();
        let size = rules + 2 + rules / 10 + 2;
        let mut group = c.benchmark_group("scaling");
        group.sample_size(10);
        group.bench_with_input(
            BenchmarkId::new("index build (discover + read + validate)", size),
            &repo,
            |b, repo| b.iter(|| repo.index().unwrap()),
        );
        let index = Arc::new(repo.index().unwrap());
        group.bench_with_input(
            BenchmarkId::new("registry build", size),
            &index,
            |b, index| {
                b.iter(|| {
                    CapabilityRegistry::builder()
                        .with_modules(builtin::modules())
                        .with_index(index)
                        .build()
                        .unwrap()
                })
            },
        );
        let ctx = repo.context().unwrap();
        group.bench_with_input(
            BenchmarkId::new("openapi document", size),
            &ctx,
            |b, ctx| b.iter(|| openapi::document(&ctx.registry, "bench").unwrap()),
        );
        group.bench_with_input(
            BenchmarkId::new("mcp listings (tools + resources), computed", size),
            &ctx,
            |b, ctx| {
                b.iter(|| {
                    let s = Surface::new(Arc::clone(ctx));
                    (s.tools_json(), s.resources_json())
                })
            },
        );
        let surface = Surface::new(Arc::clone(&ctx));
        group.bench_with_input(
            BenchmarkId::new("mcp resources/list, prepared", size),
            &surface,
            |b, s| b.iter(|| s.resources_json()),
        );
        let uri = index.objects[index.objects.len() / 2].uri.clone();
        group.bench_with_input(
            BenchmarkId::new("registry lookup by uri", size),
            &ctx,
            |b, ctx| b.iter(|| ctx.registry.by_mcp_uri(&uri).is_some()),
        );
        group.bench_with_input(
            BenchmarkId::new("benchmark projection", size),
            &ctx,
            |b, ctx| b.iter(|| BenchmarkProjection::from_context(ctx)),
        );
        group.bench_with_input(
            BenchmarkId::new("objects.search, cold", size),
            &ctx,
            |b, ctx: &Arc<Context>| {
                b.iter(|| {
                    ctx.executor.clear();
                    ctx.execute("objects.search", serde_json::json!({ "query": "scope" }))
                        .unwrap()
                })
            },
        );
        group.finish();
    }
}

criterion_group!(scaling, benches);
criterion_main!(scaling);
