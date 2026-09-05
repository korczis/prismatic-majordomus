+++
title = "The paths of the Rust executable that scale with the repository carry criterion benchmarks that build on every push, and every executable capability declares its benchmark policy, required or waived for a typed reason"
description = "The work that grows with the repository is measured, not assumed: the YAML subset and front matter parse of one object, one glob match, the index built from a filesystem walk, the registry composed over it, the OpenAPI document, one MCP resources/list and one capability call (benches/projections.rs); the router, the bridge and the shared server over a real socket (benches/shared.rs). Every benchmark is a criterion target declared in Cargo.toml without the default harness, and CI builds them on every push so a hot path cannot lose its measurement silently. Separately, every executable capability carries a benchmark policy: required with the cases its input type provides, or waived with a typed reason; a resource is waived as not_executable."
weight = 110
[extra]
claim_id = "rust-hot-path-benchmarks"
status = "guaranteed"
source = "docs/claims/rust-hot-path-benchmarks.md"
+++
{% raw %}

## What it means

The work that grows with the repository is measured, not assumed: the YAML subset and front matter parse of one object, one glob match, the index built from a filesystem walk, the registry composed over it, the OpenAPI document, one MCP `resources/list` and one capability call (`benches/projections.rs`); the router, the bridge and the shared server over a real socket (`benches/shared.rs`). Every benchmark is a criterion target declared in `Cargo.toml` without the default harness, and CI builds them on every push so a hot path cannot lose its measurement silently. Separately, every executable capability carries a benchmark policy: `required` with the cases its input type provides, or `waived` with a typed reason; a resource is waived as `not_executable`.

## How it works

`[[bench]]` entries in `apps/majordomus-cli/Cargo.toml` name each file under `benches/` with `harness = false`; `cargo bench --no-run` in `scripts/rust-check` and in the `rust` job compiles them. The `capability!` macro takes the benchmark cases from the input type's `BenchmarkCases` implementation, so an executable without cases does not compile; the registry counts `benchmark_required` and `benchmark_waived`, and `capabilities list --format json` carries the policy of every capability. `test/cases/77_rust_evidence.sh` checks the `[[bench]]` entries against the files, that every path the rule names has a benchmark, builds the benchmarks when a toolchain is present, and reads the policy of every executable back through the built executable.

## How to see it

```bash
just bench                               # every criterion target; `just bench shared` runs one
just bench projections
apps/majordomus-cli/target/debug/majordomus capabilities list --format json | jq '.capabilities[] | select(.kind != "resource") | {id, benchmark}'
apps/majordomus-cli/target/debug/majordomus capabilities list --format json | jq '.summary'
```

## What it does not cover

Numbers are reported, not asserted: CI builds the benchmarks and does not run them, and no latency budget is promised until one is measured on CI's own hardware. The declared cases of the executables run as tests today; timing every capability directly and over each transport with a baseline and a check is the benchmark projection, which is its own claim when it lands. A benchmark of the shell tool's commands is `bench` in the shell tool, not this crate.

## Why it exists

The operator asked that benchmarks exist and be enforced. A benchmark that nobody builds rots with the API it measured; one that CI compiles on every push is at least always true to the code, and one that the suite can name is one that cannot be dropped in a refactor. The rule `project.rust-cli-evidence` names the paths that must be measured; this claim and the case hold the list.
{% endraw %}
