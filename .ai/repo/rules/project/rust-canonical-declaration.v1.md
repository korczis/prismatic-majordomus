---
id: project.rust-canonical-declaration
version: 1
kind: rule
title: One canonical declaration; modules compose capabilities; the root composes modules
description: An executable capability of the Rust executable is declared once with capability!, composed into its module with module!, and the application composes modules with compose_modules!; every interface, benchmark target, cache behaviour and generated document is derived from that declaration, and editing a projection to add or change one is a defect.
statement: Add or change an external operation of the Rust executable only by changing its canonical capability or module declaration and regenerating the projections; never by editing a transport registry, an OpenAPI or Swagger definition, a benchmark inventory or a documentation table, and any repeated semantic definition across projections is a design defect unless protocol-specific information cannot be derived safely.
status: active
class: blocking
depends_on: []
tags: [rust, architecture, capabilities]
---

# Rationale

The Rust executable exposes the same things through MCP, HTTP, OpenAPI, Swagger UI, the
command line, the benchmark projection, the cache and the generated reference. A fact
that lives in two of them drifts on the first edit that forgets the other. The registry
in `apps/majordomus-cli/src/capability/` is the one place a capability exists; ADR 0004
records the decision and the alternatives refused.

# Required behaviour

A new capability is one `capability!` block in its module's file under
`apps/majordomus-cli/src/capability/builtin/`, with its typed input and output and the
input's `BenchmarkCases`. A new module is one Rust module with a `module()` built by
`module!` and one name added to `compose_modules!` in `builtin/mod.rs`. After either,
`majordomus generate` refreshes `docs/generated/` and `majordomus capabilities validate`
proves the registry, the projections and the benchmark coverage. No other file is edited
for the operation to exist everywhere it should.

# Failure behaviour

The registry refuses to build, naming the id and the provenance, on a capability composed
outside its namespace's module, a module composed twice, an invalid id, a cache policy
that keeps nothing, a cached command, or a benchmark policy that contradicts the kind;
`tests/projections.rs` fails when a projection carries an entry the registry does not or
lacks one it declares; `generate --check` fails on a stale generated file; a reviewer
refuses a change that adds a definition to a projection.

# Verification

`scripts/rust-check`; `apps/majordomus-cli/tests/{registry,projections,bench}.rs`;
`test/cases/76_capabilities_projections.sh` and `test/cases/91_canonical_architecture.sh`.
