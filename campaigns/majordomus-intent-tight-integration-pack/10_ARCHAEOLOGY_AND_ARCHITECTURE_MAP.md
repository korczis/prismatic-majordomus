# Milestone 1 — Intent architecture archaeology and integration map

Goal: discover the real implementation and produce the smallest correct integration plan.

## Audit

Search the repository for all code/data related to:

- intent
- criterion/criteria
- evidence
- milestone
- issue
- plan
- gap
- critique
- opposition
- session/task/episode
- start/preflight
- check/finish/completion
- GitHub sync/reconcile
- Cockpit routes/views/data
- API/OpenAPI/MCP capability declarations
- schemas
- generated registries
- rules/doctrines
- ADRs
- site/docs

Do not rely only on keyword search. Follow type references and call graphs.

## Produce a matrix

```text
concept | canonical source | writers | readers | projections | validators | tests | gaps
```

## Trace these flows

### Existing intent flow

```text
intent file
  → parse
  → validate
  → coverage
  → preflight
  → stage
  → criterion evidence
```

### Existing session flow

```text
start
  → context
  → task/session state
  → check
  → finish
```

### Existing completion flow

```text
completion contract
  → validators
  → findings
  → finish verdict
```

### Existing GitHub flow

```text
plan/issue
  ↔ GitHub issue/milestone
```

### Existing UI flow

```text
domain model
  → projection/data
  → route
  → Cockpit
```

Identify exactly where intent falls out of each flow.

## Design constraints

Before proposing types, inspect whether suitable types already exist.

Do not add:
- a new intent repository,
- a new evidence database,
- a new generic workflow engine,
- a new frontend store.

## Acceptance criteria

- complete architecture matrix,
- concrete list of missing edges,
- each missing edge assigned to exactly one later milestone,
- no speculative redesign.

## Deliverable

Commit/update the canonical plan/issue records for milestones 20–110 using current Majordomus planning conventions if repository policy requires it.
