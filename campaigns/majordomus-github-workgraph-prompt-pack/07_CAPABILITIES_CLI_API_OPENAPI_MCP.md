---
id: github-workgraph-phase-07
phase: 7
depends_on: [github-workgraph-phase-06]
goal: canonical capability projections across CLI/API/OpenAPI/MCP
---

# Phase 07 — Capabilities: CLI, REST API, OpenAPI/Swagger, MCP, Structured Output

Read shared contract and prior handovers.

## Objective

Expose the Work Graph and GitHub reconciliation as canonical Majordomus capabilities, once, then derive all supported transport surfaces.

The repository explicitly treats repeated semantic definitions across projections as a design defect. Enforce that here.

## Capability inventory

Infer exact naming from existing conventions. The semantic surface should cover operations equivalent to:

### Work graph
- inspect/list outcomes/milestones;
- inspect/list work items/issues;
- get graph/subgraph;
- dependencies/dependents;
- blockers;
- next ready work;
- status/explain;
- evidence/acceptance explanation;
- reverse lookup from branch/worktree/commit/PR.

### GitHub
- provider status/config diagnostics;
- observe/fetch snapshot;
- projection diff;
- reconciliation plan;
- reconcile/apply;
- mapping/adoption where explicitly allowed;
- explain drift/conflict.

Do not create twenty tiny commands if existing CLI conventions prefer nested actions or generic query capabilities.

## CLI

Human output:
- concise;
- deterministic;
- grouped;
- shows canonical IDs and external refs without confusing them;
- explains drift and blockers;
- supports `--json`/structured output via existing framework;
- dry-run should be obvious.

Machine output:
- schema-backed;
- stable;
- no ANSI;
- deterministic arrays;
- versioned if existing contracts require it.

## REST

Expose the same application services.

Do not let HTTP handlers contain reconciliation logic.

Use appropriate read vs mutation semantics. A reconciliation plan/read must not mutate. Apply endpoint must be explicit.

## OpenAPI/Swagger

Generate from canonical capability/schema machinery. Verify:
- schemas;
- examples;
- query/body parameters;
- dry-run/apply distinction;
- errors/diagnostics;
- external reference types;
- reconciliation plan operations.

Do not manually patch Swagger to "make the UI show it".

## MCP

Expose equivalent operations/tools/resources in the existing MCP system.

Key use cases for agents:
- find next ready issue;
- announce/claim relevant work if supported;
- inspect blockers;
- inspect issue traceability;
- inspect GitHub drift;
- dry-run reconciliation;
- retrieve completion/evidence explanation.

Mutation tools must follow the same authorization/safety semantics as CLI/API.

## Cross-surface contract

Write tests proving that the same canonical application operation yields equivalent domain results through:
- direct application call;
- CLI structured output;
- REST;
- MCP.

Do not compare presentation strings when semantic payload comparison is available.

## Completion and discovery

Update shell completion from canonical command/capability metadata if current CLI supports it. No independent list.

## Generation

Run `majordomus generate` or current equivalent. Check generated drift. If generating produces surprising unrelated changes, investigate rather than blindly commit.

## Gate

A new representative capability added to canonical definitions must automatically surface in every supported projection without editing duplicate registries.
