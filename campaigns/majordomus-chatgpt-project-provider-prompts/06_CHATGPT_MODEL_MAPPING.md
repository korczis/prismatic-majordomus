# Phase 6 — ChatGPT semantic model mapping

Implement ChatGPT-specific mapping on top of validated observations/transports.

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

## Map only what evidence supports

Where observable and authorized, support:
- project/workspace identity/title/metadata;
- project instructions/context metadata;
- conversations;
- message graph/order/roles/authors;
- structured message content parts;
- files/attachments;
- timestamps;
- project/conversation links;
- pagination;
- archived/deleted state;
- authorized workspace/sharing metadata.

Do not fabricate unsupported fields.

## Mapping boundary

```text
raw ChatGPT shape
  -> validated ChatGPT transport DTO
  -> semantic mapper
  -> canonical external workspace/conversation/message/file
```

Document lossy mappings.

Provider-only extensions belong in a typed extension namespace, not in generic fields with misleading names.

## Message graph

Do not blindly flatten if observed data contains branch/edit/regeneration relationships.

Preserve:
- stable node/message IDs;
- parent/child relations if available;
- deterministic canonical display traversal;
- timestamps as metadata rather than sole ordering truth.

## Content parts

Handle known structured parts defensively:
- text;
- structured/code content;
- file references;
- tool/generated metadata where observed;
- unknown kinds.

Unknown kinds must survive as typed opaque extensions or explicit diagnostics, not disappear silently.

## Files

Record upstream ID, safe metadata, relation to project/message, lazy download capability and provenance.

Never use browser cache paths as canonical file storage.

## Project instructions

If observable, treat project instructions as source data. Do not automatically overwrite `AGENTS.md`, rules or doctrines. Any import/derivation is separate and explicit.

## Tests

Create sanitized structural fixtures for:
- simple conversation;
- long/multi-message;
- branching/regeneration if observed;
- attachments;
- instructions;
- unknown content;
- pagination;
- rename;
- archive/delete;
- malformed/incompatible shapes.

Randomize source enumeration and prove deterministic canonical output.

Acceptance: CLI/API/MCP/Cockpit can consume canonical snapshots without importing browser/private DTO types.
