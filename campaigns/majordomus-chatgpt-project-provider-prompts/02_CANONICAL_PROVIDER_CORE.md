# Phase 2 — Canonical external workspace provider core

Implement the generic provider core chosen in Phase 1.

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

## Canonical domain types

Reuse existing types where possible; otherwise introduce typed equivalents of:

```text
ProviderId
ProviderCapability
ProviderSupportLevel
ProviderTransportKind
ExternalWorkspace/Project
ExternalConversation
ExternalMessage
ExternalFile/Attachment
ExternalActor
ExternalReference
ProviderCheckpoint
ProviderChange
ProviderDiagnostic
Provenance
```

Types crossing storage/API boundaries need explicit schema/serialization/versioning consistent with repo policy.

## Provider interface

Implement an async provider contract for:
- capability discovery;
- project/workspace enumeration/detail;
- conversation enumeration/detail;
- message access;
- file/attachment metadata;
- incremental sync;
- watch/change stream;
- diagnostics/health.

Unsupported capability must produce a typed unsupported response/diagnostic, never a fake empty success.

## Registry

Hook providers into the canonical registry/discovery mechanism.

Hard test:
> A new provider implementation must not require separate edits in CLI, API, Swagger, MCP, Cockpit, docs and completion.

Refactor existing manual lists if they block this.

## Determinism

Stable canonical ordering must be guaranteed for machine-discovered collections exposed to humans or stable machine consumers. Never leak map/filesystem/concurrency order.

## Generic application service

Create/reuse one provider service layer that consumers call. CLI/HTTP/MCP/Cockpit must not call transport implementations directly.

## Schema generation

Derive JSON Schema/OpenAPI/MCP schemas from canonical types/registries according to existing repo patterns. No standalone ChatGPT-only schema islands.

## Diagnostics

Use canonical diagnostic infrastructure. Include:
- provider;
- transport;
- operation;
- code/severity;
- remediation;
- safe machine metadata.

No secret payloads.

## Mock provider

Build an in-memory or fixture provider if useful. It should exercise the whole provider contract without browser or ChatGPT credentials and become the basis for cross-surface tests.

## Tests

Cover:
- registry discovery;
- duplicate provider IDs;
- capability negotiation;
- unsupported operations;
- stable IDs;
- deterministic ordering;
- checkpoint version validation;
- serialization/schema round trips;
- diagnostic redaction;
- generic service behavior.

## Documentation

Document how a future Claude/Gemini provider implements this core without per-surface registration.

Do not proceed until the provider core is fully testable without ChatGPT.
