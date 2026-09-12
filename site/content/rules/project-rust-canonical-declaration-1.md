+++
title = "One canonical declaration; modules compose capabilities; the root composes modules"
description = "One canonical declaration; modules compose capabilities; the root composes modules"
weight = 117
[extra]
kind = "rule"
slug = "project-rust-canonical-declaration-1"
identity = "project.rust-canonical-declaration@1"
status = "active"
source = ".ai/repo/rules/project/rust-canonical-declaration.v1.md"
+++
{% raw %}

## Rationale

The Rust executable exposes the same things through MCP, HTTP, OpenAPI, Swagger UI, the
command line, the benchmark projection, the cache and the generated reference. A fact
that lives in two of them drifts on the first edit that forgets the other. The registry
in `apps/majordomus-cli/src/capability/` is the one place a capability exists; ADR 0004
records the decision and the alternatives refused.

## Required behaviour

A new capability is one `capability!` block in its module's file under
`apps/majordomus-cli/src/capability/builtin/`, with its typed input and output and the
input's `BenchmarkCases`. A new module is one Rust module with a `module()` built by
`module!` and one name added to `compose_modules!` in `builtin/mod.rs`. After either,
`majordomus generate` refreshes `docs/generated/` and `majordomus capabilities validate`
proves the registry, the projections and the benchmark coverage. No other file is edited
for the operation to exist everywhere it should.

## Failure behaviour

The registry refuses to build, naming the id and the provenance, on a capability composed
outside its namespace's module, a module composed twice, an invalid id, a cache policy
that keeps nothing, a cached command, or a benchmark policy that contradicts the kind;
`tests/projections.rs` fails when a projection carries an entry the registry does not or
lacks one it declares, and when a `cli` exposure names a command the clap declaration does
not have — the command line is the one projection the registry cannot build by itself, and
therefore the one that can disagree with its declaration; `generate --check` fails on a
stale generated file; a reviewer refuses a change that adds a definition to a projection.

## Verification

`scripts/rust-check`; `apps/majordomus-cli/tests/{registry,projections,bench}.rs`;
`test/cases/76_capabilities_projections.sh` and `test/cases/91_canonical_architecture.sh`.
{% endraw %}
