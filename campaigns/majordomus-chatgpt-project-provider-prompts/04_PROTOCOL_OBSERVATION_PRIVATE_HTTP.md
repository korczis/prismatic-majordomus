# Phase 4 — Protocol observation registry and validated private HTTP fast path

Turn sanitized browser evidence into a durable compatibility layer. Do not hardcode undocumented endpoint folklore.

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

## Observation registry

Model protocol observations with fields equivalent to:

```text
provider
transport
operation candidate
route/template fingerprint
method
request schema fingerprint
response schema fingerprint
stream/event schema fingerprint
pagination/cursor hints
status/content type
first_seen / last_seen
fixture reference
compatibility state
```

Keep raw/observed shapes separate from canonical domain types.

## Compatibility states

Use typed states equivalent to:

```text
observed
candidate
validated
degraded
incompatible
disabled
```

Promotion from candidate to validated must require explicit contract evidence/tests. One request is not enough.

## Sanitized fixtures

Store tiny structural fixtures in canonical test locations:
- no cookies/tokens;
- preferably synthetic IDs/content;
- enough structure for parser/contract tests;
- versioned;
- deterministic;
- generated/redacted by tooling, not manual copy-paste of live HAR.

## Direct/private HTTP transport

Implement a fast path only when:
- operation contract is validated;
- authenticated execution context is available through approved runtime mechanisms;
- compatibility health is good;
- provider capability allows it.

Prefer request execution inside browser context if that avoids exporting auth material.

## Central transport policy

One policy decides official/private/browser selection. Commands and handlers do not each implement fallback logic.

## Failure behavior

On status/schema/auth mismatch:
1. stop trusting direct operation;
2. mark degraded/incompatible;
3. emit diagnostic;
4. fall back to browser if available;
5. capture sanitized new observations;
6. expose deterministic protocol diff;
7. never convert partial parse into canonical truth.

## Tooling

Provide canonical inspect/status/diff/validate operations and derive CLI/API/MCP/Cockpit/docs from them.

## Tests

Cover:
- route fingerprint stability;
- schema fingerprint stability;
- observation dedupe;
- secret redaction;
- candidate promotion;
- validated private fast path;
- contract break -> browser fallback;
- incompatible schema -> no canonical corruption;
- deterministic diff;
- fixture drift.

No stealth, anti-bot bypass or access-control bypass belongs here.
