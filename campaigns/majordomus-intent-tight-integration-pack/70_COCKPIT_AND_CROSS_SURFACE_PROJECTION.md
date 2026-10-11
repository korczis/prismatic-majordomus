# Milestone 7 — First-class intent view in Cockpit and consistent projections

Goal: make intent operationally understandable to humans without moving business logic into the UI.

## Backend first

Create/reuse one typed intent projection/view model from canonical state:

```text
identity
statement
stage
verdict
invariants
criteria[]
  state
  evidence
  blockers
milestones[]
issues[]
opposition
session bindings
github reconciliation
history/revision
```

Only include fields justified by current model.

This projection must be reusable by CLI structured output/API/MCP/Cockpit.

## Cockpit information architecture

Add a first-class intent surface integrated with existing Cockpit navigation/design system.

Minimum useful views:

### Intent list
- id/title/statement summary,
- stage,
- verdict,
- progress derived from criteria,
- blocker count,
- active milestone/work count.

### Intent detail
- statement,
- invariants,
- criteria with evidence status,
- milestones/issues,
- opposition status,
- active/recent sessions bound to it,
- GitHub reconciliation state,
- blockers/gaps,
- relevant evidence links.

### Traceability
Make it easy to answer:
- why does this issue exist?
- which criterion does it serve?
- what evidence satisfies that criterion?
- what blocks the intent?
- which session is working on it?
- what is drifted remotely?

## UI constraints

- no mocked data,
- no hardcoded inventory,
- no JavaScript satisfaction logic,
- no duplicated status vocabulary,
- no route-specific data model if generic entity routing already exists,
- responsive and accessible,
- use existing design stack/conventions.

## CLI/API/MCP

Ensure canonical view/status is available through existing capability system.

Do not register the same operation separately per transport.

## OpenAPI

If API is public, schema must derive from the same typed contract.

## Tests

Add:
- backend projection tests,
- route tests,
- API schema tests,
- MCP parity tests,
- Cockpit rendering tests,
- empty/satisfied/unsatisfied/stale states,
- links/address resolution.

## Cross-surface contract

For one fixture, assert equivalent:
- intent id,
- verdict,
- criterion state,
- blockers
through CLI JSON, API, MCP and Cockpit data source.

## Acceptance criteria

A human can inspect intent state in Cockpit, and the UI cannot disagree with backend semantics without a failing test.
