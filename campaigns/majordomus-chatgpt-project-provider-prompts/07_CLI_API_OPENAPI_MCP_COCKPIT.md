# Phase 7 — CLI, API, OpenAPI/Swagger, MCP and Cockpit

Expose the provider through every canonical Majordomus control-plane surface from one application service.

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

## One application layer

Canonical operations should cover concepts equivalent to:
- providers list/status/capabilities;
- provider connect/config/doctor;
- projects list/show;
- conversations list/show;
- messages/conversation content;
- files metadata;
- sync;
- watch/status;
- protocol status/diff;
- diagnostics.

Naming must follow repository conventions.

No consumer calls browser/private transport directly.

## CLI

Add discoverable human + structured output:
- deterministic ordering;
- stable IDs;
- capability-aware errors;
- safe defaults that show metadata, not entire private content;
- JSON/structured output per repo standards;
- completion derived from canonical command/provider registries.

Illustrative only:
```text
majordomus providers chatgpt status
majordomus providers chatgpt projects
majordomus providers chatgpt conversations --project <id>
majordomus providers chatgpt sync --project <id>
majordomus providers chatgpt watch --project <id>
majordomus providers chatgpt protocol status
```

## REST API

Use existing routing conventions and typed request/response models:
- pagination/checkpoints;
- support/transport metadata;
- deterministic defaults;
- capability-aware status/errors;
- event/watch endpoint only via existing event architecture.

Do not expose cookies/CDP internals.

## OpenAPI/Swagger

Generate from routes/types. No manual second catalog.

Document official vs private/browser support and watch acquisition semantics.

## MCP

Expose provider operations/resources through existing MCP registry/generation. Prefer generic provider arguments over dozens of one-off ChatGPT tools where architecture allows.

No credentials in MCP output.

## Cockpit

Integrate:
- provider status/capabilities;
- transport health;
- safe account/workspace fingerprint;
- project list;
- conversation browsing;
- sync/watch state;
- diagnostics;
- protocol compatibility;
- checkpoint/freshness;
- provenance/source badges.

Frontend calls canonical backend. It must not reimplement provider rules.

Stream watch/sync changes through existing WebSocket/event infrastructure if present.

## Generated navigation/registries

Provider UI/nav/docs links should derive from canonical metadata wherever repo supports it.

## Cross-surface contract tests

Using mock/sanitized provider data, assert canonical IDs/order/capability semantics are equivalent in:
- CLI JSON;
- REST;
- MCP;
- Cockpit backend payload.

Large conversation data must paginate/lazy-load.
