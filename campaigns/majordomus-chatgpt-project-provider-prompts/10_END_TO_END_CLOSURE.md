# Phase 10 — End-to-end migration, re-audit and closure

Finish the work. Do not leave new canonical machinery next to old duplicated paths.

## Majordomus invariants

Treat these as architectural constraints. First discover the repository's current rules/doctrines and use their canonical terminology. If an equivalent invariant already exists, extend it rather than duplicating it.

- One source of truth; projections are derived/generated.
- Infer/discover before introducing registration.
- Typed Rust/domain model before renderer-, transport- or UI-specific structures.
- Schema-backed/versioned external boundaries.
- Deterministic ordering and stable identifiers.
- Zero manually synchronized inventories across CLI/API/OpenAPI/MCP/Cockpit/docs/completion.
- New providers/capabilities/entities become visible through existing registry/discovery mechanisms automatically.
- Reuse existing diagnostics, validation, event, storage, routing, docs and generation infrastructure.
- Fix in-scope legacy duplication instead of wrapping it.
- Public implementation follows repository documentation/test/doctest rules.
- No secrets in repository, generated docs, traces, fixtures, logs, snapshots or caches.
- Raw observation != normalized entity != derived knowledge. Preserve provenance.
- Capability negotiation is explicit. Unsupported operations never masquerade as empty success.
- Private/undocumented protocol behavior is a replaceable transport detail, never the canonical domain model.
- Browser/CDP automation may operate only inside the user's already-authenticated context.
- Never bypass MFA, CAPTCHA, access controls, account boundaries or anti-abuse controls.
- Never collect unrelated browser traffic.
- Do not hardcode undocumented ChatGPT endpoints from a prompt or random gist. Observe actual authorized traffic, sanitize it, validate it, then implement against evidence.
- Prefer read-only support first. Any future write action needs explicit capability, policy and user intent.

Before modifying code, read all relevant `AGENTS.md`, `.ai/**`, `.majordomus/**`, rules, doctrines, schemas, README hierarchy, ADR policy, provider abstractions, CLI architecture, REST/OpenAPI/MCP/Cockpit code, site/GitHub Pages generators, tests, worktree conventions and canonical validation commands. Discover paths; do not assume them.

## Re-audit repository

Search again for:
- duplicate provider lists;
- ChatGPT-specific command/route/nav inventories;
- raw private DTOs leaking into generic modules;
- independent sorting;
- manual docs inventories;
- obsolete browser scripts;
- plaintext cookie/token helpers;
- old provider abstractions superseded by this work;
- TODO/FIXME from this initiative;
- generated artifacts bypassing canonical registries.

Delete or migrate in-scope dead/duplicate code.

## Zero-registration proof

Add a test-only/mock second provider and prove that one canonical registration/discovery action makes it appear where relevant in:
- provider CLI list;
- structured JSON;
- REST;
- generic OpenAPI provider operations;
- MCP;
- Cockpit provider list;
- generated docs/index;
- completion if applicable.

Do not edit each consumer to make the proof pass.

## CI-safe end-to-end proof

Without live ChatGPT credentials:
1. mock provider cross-surface tests;
2. browser transport against local fixture server;
3. sanitized protocol replay;
4. private contract failure/fallback;
5. sync/checkpoint/idempotency;
6. security/redaction;
7. docs/schema/generated drift;
8. full canonical repository gates.

## Optional live smoke

Only if an authenticated user browser is available:
- connect/status;
- enumerate authorized projects;
- identify target project;
- fetch a small conversation page;
- sync metadata;
- inspect provenance;
- verify git status contains no auth/trace/profile material.

If unavailable, explicitly say live smoke was not run. Never fake it.

## Generate/update all derived artifacts

Run canonical generators for:
- schema;
- OpenAPI/Swagger;
- MCP metadata;
- shell completion;
- provider/feature registries;
- docs/GitHub Pages;
- snapshots.

All generated output must be deterministic and drift-checked.

## Full validation

Use actual repo commands for:
- format;
- lint/static analysis;
- unit;
- integration;
- E2E;
- schema validation;
- docs/site build;
- OpenAPI drift;
- MCP;
- Cockpit;
- secret/security scan;
- generated artifact drift;
- repository-wide gate.

Fix failures caused by this work. Clearly separate unrelated pre-existing failures with evidence.

## Git/worktree hygiene

Inspect:
- status/diff/untracked;
- browser profiles;
- HAR/recordings;
- cookies/tokens;
- local absolute paths;
- large generated garbage.

No machine-local/auth material may be committed.

Follow repo commit/rebase/push policy without destroying unrelated work.

## Final acceptance

Core:
- generic provider contract;
- typed capabilities/support/transports;
- ChatGPT behind provider boundary;
- safe browser/CDP;
- sanitized versioned observations;
- validated private fast path;
- safe fallback;
- checkpointed incremental sync;
- truthful watch acquisition mode;
- idempotency;
- raw/normalized/derived separation;
- provenance.

Surfaces:
- CLI;
- structured JSON;
- REST;
- generated OpenAPI/Swagger;
- MCP;
- Cockpit;
- event/WebSocket if canonical;
- completion;
- docs/GitHub Pages.

Governance:
- rules/doctrines;
- schemas;
- skill if appropriate;
- ADR;
- README hierarchy/front matter;
- CI/gates.

Quality:
- deterministic ordering;
- cross-surface contract tests;
- replay;
- reconnect/failure tests;
- secret redaction/security gate;
- generated drift;
- canonical repo checks pass.

Extensibility:
- new provider does not need per-surface registration;
- browser observation infrastructure is reusable by other authorized web apps;
- private protocol evolution does not change canonical public contracts.

## Final report

Return:
1. root gap;
2. architecture diagram;
3. exact capabilities;
4. fixture-proven vs live-proven behavior;
5. transport/fallback policy;
6. security/privacy;
7. storage/provenance;
8. surfaces;
9. governance/docs;
10. exact validation commands/results;
11. files changed grouped by purpose;
12. live setup steps;
13. genuine upstream limitations.

No "should work" without evidence.
