# Stage 08 — Cockpit as a Real Control Plane

Turn Cockpit from a rich repository observer into a schema-driven management surface for the canonical runtime, without duplicating business logic in frontend code.

## Principles

Cockpit must consume capabilities; it must not become a second capability implementation.

The backend owns:

- capability identity/effect,
- input/output schemas,
- permissions/policy,
- validation,
- execution,
- evidence/status.

Cockpit owns rendering and interaction.

## Tasks

1. Fix/derive area/nav/route inventories so docs, enum, router and navigation cannot drift.
2. For every relevant capability, derive:
   - display metadata,
   - safe action affordance,
   - input form/schema,
   - validation errors,
   - execution status/output,
   - links to rules/claims/evidence/docs.
3. Reuse the canonical execution engine for mutations.
4. Apply permission/effect safeguards: confirmation for destructive operations, CSRF/security model as required, no secret echoing.
5. Provide generic schema-driven forms for capabilities where practical; allow specialized UI only as a projection override, not a separate domain implementation.
6. Make rules/doctrines executable/validatable from Cockpit where appropriate.
7. Make session/context/provider status manageable/observable.
8. Make issues/milestones/plan/session relationships navigable from the same canonical graph.
9. Preserve design-system consistency with GitHub Pages/project UI doctrine.
10. Add server-render/HTTP interaction tests and browser/UI tests according to existing project tooling.
11. Add a zero-registration test: a fixture capability with Cockpit exposure metadata appears automatically.

## Acceptance

- Applicable command capabilities are actually runnable from Cockpit through canonical executor.
- No frontend hardcoded registry duplicates capability/rule/provider inventories.
- Navigation cannot omit an implemented area without a failing test/generator check.
- Input/output schemas and validation are shared with API/MCP.
- Cockpit pages clearly distinguish query, safe mutation, destructive action and unavailable/degraded capability.
