# PHASE 05 — CANONICAL CAPABILITIES, DERIVES, CLI/API/OPENAPI/MCP

Canonical capability declarations and typed domain records are the ONLY semantic source. Trace the existing `capability!` registry and `majordomus generate` pipeline. Extend the existing declarations for missing operations; do not handcraft alternate REST endpoints, MCP tool tables, Swagger docs, CLI help catalogs or independent frontend operation descriptors. Respect ADR 0004, docs/CAPABILITIES.md and generator drift checks.

Projection matrix (verify names against source):

Domain entities: Intent; Criterion; Evidence; PlanProposal; Critique; Milestone; Issue; Dependency; Task/Attempt; Worker; MeshClaim; Worktree; PR; Check/Review; Release; Deployment; RuntimeHealth; AuditEvent.

Read surfaces: CLI text/JSON; REST resource operations; OpenAPI/Swagger; MCP tools/resources; UI view models; generated docs/knowledge indexes; deep links to canonical routable entity addresses (`majordomus://` and `/cockpit/objects/...` as established in `.ai/README.md`). Write surfaces: typed commands with scoped authorization, dry run, plan/preflight, idempotency, returns receipt/event link. Stream surfaces: event cursor/replay or snapshot+delta, ordered with monotonic sequence. `explain`: show upstream provenance, derived logic, missing evidence and next actions. Pagination and filtering: stable sort/cursor and canonical default ordering, no unbounded load of all events.

Contract metadata for every capability: stable id, canonical source, input/output schema, read/write effect class, authorization requirement, idempotency semantics, timeout/budget, errors, use cases, docs link, entity links and machine-discoverable UI hints where existing framework supports them. Validate defaults, optional fields, null semantics and semantic version compatibility. For dangerous effects, list dry-run/approval in discoverable metadata.

Derivation invariants:
- One underlying snapshot or event ledger -> all representations.
- Same state, ID, revision and provenance on CLI/API/MCP/UI; not necessarily identical formatting.
- New entity/operation automatically discovers into navigation/links/catalogs when its canonical kind and publication policy support it.
- No frontend hardcoded list of capability IDs/commands/routes as a second registry.
- No per-surface status math; aggregated state and next allowed transitions are computed server-side.
- Typed API error codes: unauthorized, policy_denied, stale_revision, conflict, busy, not_found, unavailable, retryable, validation_failed, incomplete_evidence; reuse existing error taxonomy if it exists.
- State-changing operations need optimistic concurrency (expected revision) plus idempotency receipt where relevant.
- Linkable deep links are canonical and stable; avoid leaking local absolute paths to public views.

Run generator; compare outputs against checked-in generated artifacts. API and MCP tool contract snapshots; CLI JSON golden comparisons; schema evolution compatibility; pagination deterministic property tests; new capability test ensuring zero manual transport edits; no contradictory statuses between surfaces. Assert that absence of worker/mesh/deploy connectivity appears as UNKNOWN/BLOCKED rather than fabricated success.

Acceptance: a single intent ID opened through CLI, API, MCP and UI points to the same canonical graph and action preflight; one newly declared capability is exposed in applicable surfaces by generation/discovery without per-surface edits.
