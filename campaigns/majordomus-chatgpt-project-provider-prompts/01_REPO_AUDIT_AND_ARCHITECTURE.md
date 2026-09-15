# Phase 1 — Deep repository audit and architecture decision

You are implementing a ChatGPT Project provider in `prismatic-majordomus`.

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

## First task: discover the actual architecture

Audit:
- current LLM/provider/adapters;
- CLI command/domain organization;
- REST router/application services;
- OpenAPI generation and Swagger;
- MCP tools/resources/prompts registry;
- Cockpit data model, API client and WebSocket/SSE/event paths;
- event bus/event envelope/store;
- storage and migrations;
- session-context, handovers, prompts, knowledge and provenance;
- schemas/front matter in `.ai/**` and `.majordomus/**`;
- completion and Just command discovery;
- diagnostics/doctor;
- secrets/config/keychain conventions;
- any existing browser/CDP/Playwright tooling;
- tests/fixtures/snapshots/E2E;
- GitHub Pages/Zola generation;
- rules/doctrines/skills/ADRs and how they are actually enforced;
- worktree/branch doctrine.

Create an internal audit matrix:

```text
concern | canonical source | implementations | consumers | duplication | reuse/refactor | tests/gates
```

Do not stop at the first similar abstraction. Determine whether a generic external workspace/provider abstraction already exists and should be extended.

## Architecture decision

Choose the narrowest generic contract that supports concepts equivalent to:

```text
capabilities()
list_workspaces/projects()
get_workspace/project()
list_conversations()
get_conversation/messages()
list_files()
get_file metadata/content capability
sync(checkpoint)
watch/changes()
health/diagnostics()
```

Transport is separate from provider semantics:

```text
provider domain
   ^
   |
official transport
browser/CDP transport
private HTTP transport
```

## Identifier/provenance model

Define:
- provider namespace;
- upstream workspace/project ID;
- upstream conversation/message/file IDs;
- local canonical IDs;
- observation IDs;
- content/version hash policy;
- deletion/tombstone semantics;
- checkpoint schema/version;
- source/support/transport metadata.

Never use display title alone as identity.

## Support-level model

Represent official/supported, browser-observed, private/undocumented, imported/exported and derived states as typed metadata, not prose strings.

## Security decision

Decide where browser executable/profile references/CDP endpoints live according to repo conventions.

Forbidden:
- committed cookies;
- copied bearer/session tokens;
- raw auth headers;
- browser profile archives;
- unsanitized HARs.

## ADR

Follow the existing ADR policy. If this qualifies, create/update the ADR now.

## Implement preparatory refactors

If the audit shows duplicated provider registries or application-service gaps that would force a bad ChatGPT-specific implementation, fix the smallest necessary architecture first.

## Exit criteria

Before Phase 2:
- architecture is grounded in actual repo;
- provider core boundary is agreed by code/docs/ADR, not just chat text;
- no parallel existing abstraction is being reinvented;
- canonical gates still pass.
