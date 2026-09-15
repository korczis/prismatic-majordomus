# Phase 8 — Documentation, rules, doctrines, schemas, skills and GitHub Pages

Make the architecture self-explaining and mechanically enforced.

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

## Documentation

Update canonical README hierarchy/site with:
1. purpose/architecture;
2. support/transport levels;
3. security boundary;
4. browser/CDP setup;
5. capability model;
6. sync/watch/checkpoints;
7. raw vs normalized vs derived data;
8. provenance;
9. protocol observation/compatibility;
10. doctor/troubleshooting;
11. adding a second provider;
12. privacy/retention;
13. upstream-change failure modes.

Generated tables for providers/capabilities/commands/API/MCP should come from registries, not copied Markdown.

## GitHub Pages

Integrate into existing Zola/GH Pages navigation and landing/feature surfaces according to repository doctrine.

New feature discoverability should be automatic if the repo has a generated feature catalog.

## Rules/doctrines

Use the repo's actual distinction. Enforce at least:

> Provider facts are discovered once, typed once and projected everywhere. Consumer-specific provider inventories are forbidden.

> Private upstream DTOs/endpoints are transport details and cannot define canonical API/storage/domain contracts.

> Provider observations are sanitized before persistence. Auth secrets are forbidden in fixtures, logs, snapshots and generated artifacts.

> Raw observations, normalized entities and derived knowledge are distinct provenance-linked layers.

> Capabilities/support levels are explicit and machine readable.

> Stable machine/human collections are deterministically ordered.

> New providers participate in surfaces via canonical registry/generation, not per-surface registration.

> Browser automation operates only in an authorized context and does not bypass security controls or collect unrelated browsing data.

## Machine gates

Integrate with existing check/gate/CI mechanisms to detect:
- duplicate provider inventories;
- provider schema drift;
- generated docs/OpenAPI drift;
- unsanitized fixture/log patterns;
- missing docs/tests/schema required by repo doctrine;
- frontend/API references to noncanonical provider IDs;
- nondeterministic generated ordering.

Do not create a redundant parallel CI universe.

## Skill

If Majordomus uses executable/agent skills, add or extend a provider-integration skill covering:
- how to add a provider;
- safe browser observation;
- fixture sanitization;
- contract/replay tests;
- private protocol upgrades;
- zero-registration surfaces.

Only if skills are an actual repo mechanism.

## ADR

Update the ADR from Phase 1 with final implementation choices.

## Acceptance

A new developer can connect an authorized browser, inspect status, sync/watch, find stored provenance, diagnose protocol changes and add another provider without editing every consumer.
