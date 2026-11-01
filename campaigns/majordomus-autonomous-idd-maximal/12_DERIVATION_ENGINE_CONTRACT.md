# Phase 12: Derives, Projection, API, MCP, CLI and Docs Convergence

## Fundamental rule
Canonical domain declaration -> typed capability -> schema & metadata -> transport adapters -> UI and docs projection. A new capability/entity must propagate without independent consumer registration. Inspect `docs/CAPABILITIES.md`, ADR 0004 and the actual `capability!` grammar; implement only additions missing in source.

## Capability descriptor fields (conceptual, reuse current)
- id/version/domain/action and stable route/entity address
- typed input & output and typed error schema
- read/query vs authorized effect and replay/idempotency semantics
- required capability/permissions and environmental preconditions
- explain/dry-run support if applicable
- pagination/filter/sort/event cursor semantics
- lifecycle/stability/deprecation metadata
- topic/event subscription metadata if actually supported by registry
- CLI presentation hints and UI interaction schema where canonical conventions allow
- examples/use case IDs, test links and documentation references
No ad-hoc JSON schema copies in frontend or MCP.

## Desired derived operations (names illustrative ONLY)
Intent: list, show, draft, validate, propose-plan, critique-plan, plan-diff, approve, launch, pause, resume, cancel, replan, explain.
Plan: list revisions, show DAG, compute readiness, inspect rationale, detect overlap.
Milestone: list/show/progress/waves/blockers/coverage.
Execution: list/show/events/attempts/checkpoint/cancel/retry/explain, typed stream cursor.
Mesh: peer/node/capabilities/health/claims/handover/review/assignment explanation.
Worktree/PR: list, readiness, gate result, integration plan, authorized merge, cleanup preview.
Delivery: release candidate, version explanation, release, deployment, verify, rollback eligibility.
Evidence: list/show/validate/freshness/provenance/criterion verdict/intent satisfaction.
Governance: policy/effective permissions/gates, diagnostics.
These are domain capabilities, not mandatory new commands; reuse and extend equivalents.

## Cross-surface contract matrix
For EACH added or modified capability, prove:
1. canonical registry discovery returns one unique ID;
2. CLI help/list and invocation use the same typed inputs and diagnostics;
3. REST route exists from generated router and respects HTTP methods/auth;
4. OpenAPI advertises identical schema, examples, status/errors/security;
5. MCP discovery returns relevant tool/resource, equivalent schema and permission;
6. UI derives or consumes same operation without duplicated state logic;
7. docs and use-case registry link canonical operation and examples;
8. unauthorized actor rejected consistently across transports;
9. no surface leaks local operational secrets;
10. deprecation/version semantics consistent.

## API
- Stable versioned identifiers and entity addresses.
- Query endpoints use cursors, stable total order, bounded page sizes and consistent filtering.
- Mutation endpoints accept request/correlation ID, idempotency key and concurrency precondition.
- Return typed `operation_ref` for long running effects; never block HTTP request until an agent finishes.
- `202 Accepted` means accepted, not completed. Return links to status/stream.
- `409` for conflicting revision or claim, `403` for denied side effect, `422` for invalid intent and `503` for unavailable backend as appropriate to project conventions.
- Strict redaction and least-privilege; no public execution endpoint simply because OpenAPI generated it.

## Streams
- One canonical ordered event model for SSE/WebSocket/MCP subscriptions where supported.
- Resume token/cursor, gap detection, snapshot+delta resync, heartbeat.
- Bounded buffer with backpressure and per-client limits.
- Identify event version and global/stream cursor to reconcile duplicates.
- Explicit disconnect/reconnect semantics; no client-side guess at authoritative state.
- Prefer existing streaming transport; add WebSockets only for clear bidirectional need.

## Schema evolution
- Version payloads and semantic identifiers; migration tests.
- Additive fields with defaults only when semantics are safe.
- Never silently change enum meaning, event order or artifact URLs.
- Contract tests sample same entity across CLI JSON, HTTP, MCP and UI projection.
- Generated OpenAPI/docs drift gate must fail when canonical source and output disagree.

## Discovery test
Create one fixture capability and one new criterion in a disposable test repo. Assert automatic discoverability, entity links, schema, OpenAPI, MCP, UI navigation/action and docs where publication policy permits. The test must not need per-surface catalog edits.
