# Phase 16: Canonical Documentation, GitHub Pages, UX Writing and Adoption

## Canonical references
Inspect `docs/README.md`, `docs/GITHUB_PAGES_ARCHITECTURE.md`, `docs/PAGES_STATUS.md`, `docs/SITE_CLAIMS.md`, `docs/PRODUCT.md`, `docs/DOGFOODING.md`, `docs/WHY.md`, docs/SCHEMAS.md and existing generated docs pipeline. Follow front matter and README composition; do not edit generated files as sources.

## Documentation architecture
- Concept: what is an intent, acceptance criteria, plan revision, evidence and derived satisfaction.
- Architecture: canonical graph, event journal, scheduler, mesh, effect reconciler and projection.
- Quickstart: one command installation where existing install architecture permits, `majordomus ui`, first sandbox intent.
- Operator guide: policies, budgets, approvals, stop/pause/resume, safe retry and incident diagnosis.
- Developer guide: add capabilities/criteria/workers without per-surface registration.
- Mesh guide: peer identity, authentication, claims vs locks, lease recovery and health.
- Security: authorization tiers, secret hygiene, audit log, trust boundaries.
- UI guide: intent wizard, milestone DAG, mesh inspector, live execution, delivery.
- API/CLI/MCP: generated references from canonical capability metadata.
- Troubleshooting: blocked criteria, stale evidence, conflicting worktrees, failed PR/deploy, split-brain.
- Performance and operations: metrics definitions, traces, resource budgets and known limits.
- Migration: compatibility notes and old-to-new commands if public contracts changed.

## Publication safety
- Generated public pages contain only explicitly publishable fields and canonical references.
- No local `.ai/local`, secrets, internal peers, private GitHub data, raw session logs, deployment credentials or privileged action tokens.
- Static GitHub Pages must not pretend to be a live authenticated orchestration UI.
- Docs examples clearly mark sandbox sample vs live observation.
- All "fully autonomous", "verified", "real time", "safe" claims require linked concrete evidence with date/version.
- Site deploy is not equivalent to successful runtime deploy.

## Docs derivation assertions
New canonical capability or published entity appears in generated reference indexes and navigation without manual duplication.
Broken entity links fail CI.
Examples are executable use cases where possible; generated command shapes match actual CLI and MCP contracts.
Drift checker compares source digest with generated artifact digest.
Release notes and site version reflect actual released SHA/tag.

## Adoption / DX
- Minimal first-run path, typed diagnostics for absent providers and missing auth.
- A one-page first intent walkthrough: create, inspect inferred acceptance, approve sandbox policy, run, inspect evidence.
- Discoverability from top-level UI and CLI; consistent names and routes.
- Show limitation badges honestly: local-only, mesh-enabled, sandbox-only, policy-blocked, not supported.
- Avoid promising zero configuration when credentials/policies are in fact required.
