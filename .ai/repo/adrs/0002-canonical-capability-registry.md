---
schema: adr/v1
id: adr-0002
kind: adr
title: A canonical capability registry, with MCP, HTTP, OpenAPI, Swagger UI, the CLI and the reference as projections
status: accepted
date: 2026-09-05
tags:
  - registry
  - projections
  - architecture
provenance:
  origin: authored
---

# 2. A canonical capability registry, with MCP, HTTP, OpenAPI, Swagger UI, the CLI and the reference as projections

Extends ADR 1.
## Context

ADR 1 added a Rust executable with one command, `mcp`, and a hand-written table of four
tools and a resource listing inside the MCP code. The operator asked for the same
capabilities over HTTP with an OpenAPI document and Swagger UI, for introspection from the
command line, and for generated reference documentation, with one hard invariant: a
capability is defined once, and every interface is derived from that definition, so that
no two interfaces can drift apart silently. The repository's own doctrine says the same
about generated files (`project.derived-files-regenerated`) and about numbers in prose.

## Decision

- **One model.** A `Capability` is a typed descriptor: canonical id, kind (`query` or
  `resource`), title, description, canonical input and output schemas, provenance,
  exposure (MCP tool and/or resource, HTTP method and path, CLI path — each explicit,
  none inferred), stability, tags. A query carries one typed handler; a resource is read.
- **Two sources, one registry.** Builtin executables are composed explicitly in
  `capability/builtin.rs` through a `macro_rules!` that only fills the descriptor and wraps
  the typed function; no procedural macro, no linker registry, no code generation. Every
  object of the index becomes a resource capability. Both meet in `CapabilityRegistry`,
  built at one place (`app.rs`), which refuses duplicate ids, duplicate MCP names and
  URIs, duplicate HTTP routes, duplicate CLI paths, malformed exposures, and an
  executable exposure on a planned or unsupported capability, naming every party.
- **Schemas from types, at run time from data.** Input and output schemas of executables
  come from the Rust types (`schemars`); the metadata contract of every declarative kind
  is a JSON Schema (draft 2020-12) read at run time from `share/schemas/`, validated with
  a JSON Schema validator; `share/kinds.yaml` says how each kind is read. A repository
  adds kinds and schemas under `.ai/repo/knowledge/`; it cannot redefine a distributed one.
- **Projections.** MCP (`mcp/surface.rs`), HTTP routes with GET query binding and POST
  body binding (`http/router.rs`), OpenAPI 3.1 with `operationId` = canonical id and
  `x-majordomus-*` extensions (`http/openapi.rs`), a Swagger UI shell that loads
  `/openapi.json` and embeds nothing (`http/swagger.rs`), the `capabilities` commands
  dispatching through the registry's CLI exposure, `docs/generated/openapi.json` and
  `docs/generated/capabilities.md` from one generator, and the shell tool's
  `share/allow/*.txt` derived from the schemas. `majordomus generate --check` compares
  every committed projection with the registry and exits 10 when one is stale.
- **HTTP is loopback and read-only.** `serve` binds 127.0.0.1 unless told otherwise,
  has no authentication because it exposes nothing that is not already on disk to the same
  user, and lives as long as its stdin when that is a pipe.

## Invariants

1. Canonical ids are unique across both sources.
2. No projection carries an entry the registry does not hold, and none lacks an entry the
   registry declares for it (`tests/projections.rs`).
3. Every schema a projection shows originates in the canonical schema of the descriptor.
4. Generated output is deterministic: sorted maps, no timestamps, no absolute paths.
5. Committed projections are reproducible and their staleness is detected.
6. A projection cannot redefine a capability's semantics: MCP and HTTP call the same
   handler (`tests/http_serve.rs`, cross-protocol parity).
7. A planned capability may be listed but is never executable through any projection.

## Alternatives rejected

- **OpenAPI as the source of truth.** It cannot express MCP resources, CLI paths or the
  declarative layer, and it would make a documentation format the owner of behaviour.
- **MCP as the source of truth.** Same objection, from the other side; and MCP has no
  notion of a route.
- **Hand-maintained parallel tables** for tools, routes and docs: the drift the operator
  named as the problem.
- **Annotation-driven OpenAPI** (`utoipa`-style attributes on handlers) duplicating path,
  description and operation id beside the descriptor; and a second HTTP stack (`axum` with
  an async runtime) for six loopback routes, where a synchronous `tiny_http` loop suffices.
- **A runtime-only registry** with no committed projection: reviewable diffs of the
  interface would not exist.
- **Procedural macros or link-time registration** (`inventory`, `linkme`): composition
  would stop being readable in one file.
- **Regex allow-lists as the contract** for declarative kinds: not typed, not standard,
  and not extendable by a repository without teaching the code a new list.

## Consequences

The registry, the id grammar, the URI scheme, the tool names, the route paths, the
diagnostic codes and the two schema files become compatibility surfaces. Two executables
now read the layer and the shell tool's allow-lists are generated from the schemas, so the
schemas are the contract and the lists are a projection; the shell tool is not changed.
The repository's "Intentionally Absent" list keeps refusing daemons and registries of
workers; this registry indexes the tool's own surface and nothing else.

## Testing strategy

Black-box first: the compiled binary is spawned for the CLI, MCP over pipes, HTTP over a
loopback socket, and `generate --check`; the registry's invariants and the propagation of a
change in one descriptor to every projection are library tests over fixture registries;
`test/cases/76_capabilities_projections.sh` reads one capability back through every
interface from a repository the shell tool's `init` wrote. CI runs `cargo fmt --check`,
`clippy -D warnings`, `cargo test` with doctests, `cargo doc` with warnings denied,
`generate --check`, `capabilities validate` and a line-coverage threshold.
