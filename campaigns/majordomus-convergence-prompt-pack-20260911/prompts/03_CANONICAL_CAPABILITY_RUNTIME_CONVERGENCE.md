# Stage 03 — Canonical Capability Runtime Convergence

This is the central migration. Eliminate dual ownership between shell development lifecycle logic and the typed Rust/capability runtime.

## Starting point

Stage 01 should have generated the authoritative list. The 2026-09-11 snapshot had accepted `backing:*` debts for development commands including `adr`, `checkpoint`, `decision`, `evidence`, `finish`, `handover`, `init`, `migrate`, `plan`, `question`, `rules`, `session`, `start`, `update`, and `usecase`.

Do not assume the list is unchanged.

## Target architecture

For every public development operation:

```text
command metadata / user intent
          ↓
typed capability input
          ↓
one domain handler/service
          ↓
typed result + diagnostics/evidence
          ↓
CLI adapter
HTTP projection
OpenAPI projection
MCP projection
Cockpit action/view
workflow/agent use
```

No shell/domain duplicate.

## Migration method

For each operation:

1. characterize current behavior from tests + source, including filesystem writes, git changes, validation, output and failure semantics,
2. write/strengthen black-box characterization tests before moving logic,
3. define/reuse canonical domain types,
4. define command capability metadata including effect, permissions, input/output schema, idempotence/retry semantics where relevant,
5. move implementation to the canonical Rust/domain layer,
6. expose it through the existing capability executor,
7. convert shell/legacy CLI to a thin compatibility adapter calling canonical runtime, or remove it according to compatibility policy,
8. wire structured CLI, HTTP, OpenAPI, MCP and Cockpit where relevant,
9. migrate tests so behavior is proven at handler and E2E levels,
10. remove duplicate parsers/writers/validation paths,
11. remove the corresponding baseline debt.

## Critical semantics to preserve/prove

For lifecycle commands such as session/start/checkpoint/handover/finish:

- atomicity / crash safety,
- canonical IDs,
- provenance,
- current session identity,
- append-only versus mutable semantics,
- concurrency/locking behavior,
- idempotence where expected,
- filesystem paths derived from one model,
- exact diagnostics and recovery path.

For plan/ADR/rules/decision/question/usecase:

- schema validation,
- referential integrity,
- issue/milestone/session linkage,
- deterministic generated projections,
- no manual duplicate indexes.

## Compatibility

Do not preserve architecture debt under the name “compatibility”. A compatibility command may remain, but it must delegate to canonical implementation and have a removal/deprecation policy if appropriate.

## Tests

For each migrated operation add:

- unit/domain tests,
- invalid input tests,
- state transition tests,
- repeated/idempotence tests where applicable,
- filesystem/git integration tests,
- CLI black-box test,
- HTTP/MCP contract tests if surfaced,
- Cockpit action test if surfaced,
- parity test showing adapters produce the same canonical result.

## Acceptance

- `development-semantics` backing debt is zero.
- No Cockpit/shell layer owns a duplicate development domain mutation.
- All migrated capabilities are discoverable from canonical metadata.
- New operations require one registration/definition at the canonical layer and derive downstream projections.
- Relevant generated OpenAPI/docs are synchronized.
- Full local tests/gates are green.
