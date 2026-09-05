---
id: project.interfaces-are-projections
version: 1
kind: rule
title: External interfaces are projections of one capability definition
description: A capability is defined once, as a typed descriptor in Rust or as a declarative object with its schema in the layer, and MCP, HTTP, OpenAPI, Swagger UI, CLI introspection and the generated reference are derived from it.
statement: A capability is defined once and every external interface is derived from that definition; an interface that carries a definition of its own is a bug.
status: active
class: blocking
depends_on: []
tags: [architecture, capabilities, rust]
---

# Rationale

The Rust executable exposes the same things through several interfaces. A tool table in the MCP code, a route table in the HTTP code, a hand-written OpenAPI document and a Markdown inventory would be four copies of one fact, and copies drift the moment one is edited. The registry in `apps/majordomus-cli/src/capability/` is the one place a capability exists; `docs/CAPABILITIES.md` says what is canonical, what is derived, and what is not authoritative.

# Required behaviour

An executable capability is one typed descriptor with one handler, composed into the registry in `capability/builtin.rs`. A declarative object is a file the layer's `sources.yaml` maps to a kind whose reading `share/kinds.yaml` and `share/schemas/` declare, read at run time; a repository adds kinds and schemas under `.ai/repo/knowledge/` and never edits Rust for another object of a known kind. MCP tools and resources, HTTP routes, the OpenAPI document, the Swagger UI shell, the `capabilities` commands and `docs/generated/` are projections built from the registry when asked; none of them declares a name, a description, a schema or a route of its own. The shell tool's allow-lists under `share/allow/` are generated from the schemas.

# Failure behaviour

A reviewer decides this rule for a change that adds a definition to a projection; the crate's suites decide the machine-checkable part: `tests/projections.rs` fails when a projection carries an entry the registry does not, or lacks one it declares, and `majordomus generate --check` fails when a committed projection differs from the registry.

# Verification

Review, `cargo test --manifest-path apps/majordomus-cli/Cargo.toml`, and `test/cases/76_capabilities_projections.sh`, which builds the executable, changes nothing but data, and reads the same capability back through every interface.
