# Phase 5 — Incremental sync, watch/events, idempotency and provenance

Make this a continuously useful control-plane integration, not a one-shot export script.

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

## Sync contract

Implement/reuse semantics equivalent to:

```text
SyncRequest(provider, project scope, checkpoint, reconciliation policy)
SyncBatch(changes, next checkpoint, completeness, transport, diagnostics)
```

## Change model

Represent explicit change kinds:
- created;
- updated;
- deleted/tombstoned;
- moved/relinked where meaningful;
- attachment added/removed;
- metadata changed.

Never infer deletion from absence in a partial page.

## Checkpoints

Checkpoint must be:
- provider namespaced;
- versioned;
- serializable;
- safe to persist;
- invalidated appropriately on account/workspace identity change.

If upstream has no reliable cursor, implement deterministic reconciliation from stable IDs/hashes/timestamps and expose completeness limitations.

## Watch/subscription abstraction

Expose one watch/change-stream abstraction with truthful acquisition mode:
- push;
- stream/SSE;
- WebSocket;
- browser-network-observed;
- poll/reconcile;
- manual.

Consumers should not care which transport generated an event, but provenance must.

## Event integration

Reuse Majordomus event envelope/bus/store if present. Do not create a ChatGPT-only event universe.

Every event carries:
- provider;
- project/workspace;
- upstream entity ID;
- observation/sync batch ID;
- transport;
- observed/fetched timestamp;
- canonicalized timestamp;
- schema versions.

## Storage layers

Enforce separation:

```text
raw observations/source mirror
  != normalized provider entities
  != derived Majordomus knowledge/session artifacts
```

Derived knowledge always links back to source message/conversation/project.

## Idempotency

Repeated sync, overlapping pages, reconnect replay and watch+reconciliation overlap must not duplicate canonical entities/events.

## Bounds/backpressure

Use bounded queues, payload limits, batches, rate controls and bounded retry. A busy ChatGPT tab must not flood Majordomus.

## Integration hooks

Expose explicit import/derivation hooks into existing:
- session-context;
- handovers;
- prompt/decision extraction;
- knowledge indexing;
- search.

Do not automatically turn every observed message into a rule or session record.

## Tests

Prove:
- checkpoint resume;
- same batch twice;
- overlapping pages;
- out-of-order/replayed events;
- watch then reconcile;
- updates/deletes;
- attachment late arrival;
- deterministic replay;
- provenance retention;
- type/API separation between raw/normalized/derived.
