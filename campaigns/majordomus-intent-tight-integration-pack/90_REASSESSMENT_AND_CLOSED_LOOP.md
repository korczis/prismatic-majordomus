# Milestone 9 — Reassessment and closed-loop intent execution

Goal: after work/evidence changes, Majordomus automatically recomputes what the intent now means and what remains.

## Trigger points

Reassessment should occur through existing lifecycle at meaningful boundaries:

- session start,
- check,
- finish,
- evidence addition,
- plan revision,
- GitHub reconciliation,
- relevant revision/head change,
- explicit intent status/reassess command if useful.

Do not create an uncontrolled background daemon unless repository architecture already has one.

## Reassessment algorithm

```text
load intent
  ↓
validate links
  ↓
resolve current plan
  ↓
evaluate opposition freshness
  ↓
evaluate evidence freshness
  ↓
evaluate criteria
  ↓
evaluate intent
  ↓
derive remaining gaps/work
  ↓
publish canonical view/findings
```

## Reopening

If previously satisfied evidence becomes stale or an invariant fails:
- intent must no longer present as satisfied,
- stage/verdict updates through derivation,
- affected surfaces show the change,
- completion history remains historical evidence, not rewritten.

## New gap

When reassessment discovers an unmet criterion with no work:
- produce a typed gap/finding,
- do not silently create executable work unless policy explicitly permits automatic drafting,
- optionally feed desired-outcome/plan draft machinery.

## Plan evolution

New work must:
- map to criterion(s),
- undergo required opposition,
- respect dependency graph,
- become visible in coverage.

## Tests

Prove:
- satisfied → revision changes → stale/unsatisfied,
- stale evidence replaced → satisfied again,
- new invariant violation,
- orphan criterion produces gap,
- plan revision invalidates old opposition,
- UI/API update from canonical reevaluation,
- no historical record rewritten.

## Acceptance criteria

Intent is not a one-time static record.
It is a derived live state over current plan + evidence + repository revision.
