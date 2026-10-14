# Milestone 5 — Make completion consume intent satisfaction

Goal: `finish`/completion policy must refuse a claim of completion when the governing intent criteria are not satisfied.

## Required flow

```text
session/task completion request
  ↓
existing completion contract
  +
resolved session intent binding
  ↓
canonical satisfaction engine
  ↓
completion findings
  ↓
allow/refuse
```

## Key rule

No intent-governed session may successfully complete with outcome `completed` if required criteria it serves or governing completion criteria remain unsatisfied according to canonical policy.

Determine scope carefully:
- issue may serve subset of criteria,
- a single session may not be responsible for whole intent,
- issue completion and intent completion are not necessarily identical.

Model this explicitly.

## Distinguish

At minimum distinguish:
- session/work-item complete,
- milestone complete,
- intent satisfied.

Do not collapse them.

A session may legitimately finish its scoped work while the intent remains open, but:
- it must record evidence for what it claims,
- it must not declare the entire intent satisfied.

## Evidence write path

Completion should:
- collect/record verification evidence using current mechanisms,
- attach evidence to served criteria where appropriate,
- re-evaluate satisfaction,
- report what remains.

Avoid automatic evidence attachment if provenance is ambiguous.

## Refusal UX

A refusal must say:
- which criterion blocks,
- why,
- evidence missing/stale/invalid,
- reproducible next action where possible.

## Session close

On successful scoped completion:
- session can close,
- intent may remain active,
- current plan stage updates only through derived state.

## Tests

Cover:
- session work complete but intent incomplete,
- criterion evidence missing,
- stale evidence,
- evidence recorded on finish,
- subset-of-criteria issue,
- final issue satisfies final criterion,
- intent becomes satisfied without manual status edit,
- regression causes prior satisfaction to become stale/unsatisfied.

## Acceptance criteria

Completion policy becomes a consumer of the canonical satisfaction engine and never owns an independent intent verdict.
