# Prompt 10 — API, OpenAPI/Swagger, MCP, CLI, Cockpit Parity

## Mission

Make transport parity mechanically provable.

The repository already has a canonical capability registry and projection architecture. Strengthen it so IDE features cannot drift between surfaces.

## Audit every new/changed capability

For each:
- canonical id;
- module;
- kind/effect;
- stability;
- input schema;
- output schema;
- handler;
- benchmark cases;
- cache policy;
- HTTP exposure;
- CLI exposure;
- MCP exposure;
- Cockpit discoverability;
- generated docs/reference.

## HTTP

All browser-operable semantics must use canonical HTTP routes derived from capability exposure.

No `/cockpit/doThing` business endpoint if `/api/v1/...` capability owns the operation.

## OpenAPI

Generate:
- paths;
- methods;
- request schema;
- response schema;
- operation IDs;
- examples;
- descriptions;
- error schema;
- tags/grouping;
- stability/deprecation if modeled.

OpenAPI must not define semantics absent from the registry.

## Swagger

Swagger is a projection of generated OpenAPI.

Ensure:
- current server exposes it;
- schemas are complete enough to invoke operations;
- examples come from canonical cases where appropriate;
- no hand-written operation duplication;
- links from Cockpit and GH Pages are derived.

## MCP

Expose agent-useful functionality without creating unsafe mutation shortcuts.

Verify:
- tool/resource names;
- schemas;
- read-only/destructive annotations/hints if protocol layer supports them;
- errors;
- execution linkage;
- peer/session context integration.

## CLI

Prefer capability projection.
Keep CLI-local commands only when justified by current architecture.

## Parity tests

Build a generated matrix:

```text
capability id
CLI path?
HTTP route?
MCP tool/resource?
Cockpit generic discoverability?
OpenAPI operation?
docs reference?
waiver/reason?
```

The denominator is derived from registry declarations.

Validation fails on unexplained holes.

## Round-trip contract tests

For representative capabilities:
- invoke direct executor;
- CLI;
- HTTP;
- MCP;
- Cockpit route/runner;
- compare normalized semantic output.

Transport framing may differ. Domain result may not.

## Acceptance

No independently maintained parity table.
The matrix/report itself is generated from canonical descriptors and introspection.
