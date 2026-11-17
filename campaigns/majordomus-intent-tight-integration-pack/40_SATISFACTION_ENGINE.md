# Milestone 4 — One canonical satisfaction engine

Goal: create one authoritative evaluator for criterion and intent satisfaction.

## Principle

Evidence decides satisfaction.
Status fields do not.

## Required model

For each criterion, canonical evaluation returns typed state such as:

```text
Satisfied
Unsatisfied
Unknown
Stale
Invalid
```

Use the repository's existing vocabulary if already defined.

The result should explain:
- criterion,
- evidence considered,
- evidence rejected,
- revision judged,
- reason,
- blocking gaps.

## Intent verdict

Intent-level satisfaction is derived from criterion states plus invariants/policy.

Do not simply:
`all(issue.status == done)`.

Do not use milestone closed state as proof.

## Evidence freshness

Integrate current evidence-revision semantics:
- current evidence counts,
- stale evidence is named,
- invalid/unresolved evidence cannot satisfy,
- evidence provenance is retained.

## Invariants

Determine whether intent invariants:
- are constraints checked continuously,
- are acceptance conditions,
- or both.

Use current intent schema semantics.
Do not invent behavior contradicting existing docs/types.

## API design

Create/reuse a reusable evaluator called by:
- `intent status/show`,
- finish/completion,
- Cockpit,
- API/MCP,
- GitHub reconciliation diagnostics.

No local recomputation.

## Determinism

Same repository state must produce byte/structurally stable satisfaction output.

Test shuffled evidence enumeration if relevant.

## Tests

Cover:
- all criteria satisfied,
- one unsatisfied,
- unknown evidence,
- stale evidence,
- duplicate/ambiguous evidence links,
- invariant failure,
- no criteria edge case according to schema,
- revision change,
- restored current evidence,
- stable repeated evaluation.

## Acceptance criteria

Search the repository after implementation and remove/replace duplicated satisfaction logic.

There must be exactly one semantic owner.
