---
schema: adr/v1
id: adr-0113
kind: adr
title: Satisfaction is explained by the run that was judged, an optional criterion holds nothing back, and a failing guard keeps an intent unsatisfied
status: proposed
date: 2026-10-08
tags:
  - intent
  - evidence
  - satisfaction
  - governance
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
  - file:.ai/repo/adrs/0112-the-opposition-to-a-plan-is-executed-and-revision-bound.md
  - rule:project.derived-once
  - rule:project.completion-never-decides-satisfaction
  - file:apps/majordomus-cli/src/intent.rs
  - file:.ai/repo/project/intents/intent-satisfaction.yaml
  - test:test/cases/965_a_failing_guard_keeps_an_intent_unsatisfied.sh
---

# 113. Satisfaction is explained by the run that was judged, an optional criterion holds nothing back, and a failing guard keeps an intent unsatisfied

## Context

ADR 0070 derives a criterion's state from the evidence ledger and ADR 0107 derives a verdict
from the criteria alone. Three things were missing above that.

The derivation threw its reasons away. The evidence module's judgement names the inputs that
changed since the run it judged and gives a sentence; the intent engine kept one word, so a
reader told a criterion was stale had to re-derive why.

Every criterion was required. An intent could not record something it would like to become
true without letting it block the verdict forever.

And the invariants were prose. They were printed to the worker and judged by nothing: what an
intent said must stay true could stop being true while the intent read satisfied.

## Decision

**A judged criterion carries its evaluation.** `IntentCriterion.evaluation` is the latest
recorded run of the criterion's evidence — its commit, working tree, outcome and time — with
the inputs that changed since and the evidence module's own sentence. For a test it comes from
the freshness judgement; for a claim, from the claim's proof. It is derived with the state it
explains and stored nowhere, and it is absent when nothing was judged. "Considered" is one
run, the latest, because that is the only run the judgement reads. No rule name or policy
version is added: the `proof` word a criterion already carries names the judgement, and a
second vocabulary in the intent engine would be a second table.

**A criterion may be optional.** `optional: true` on a criterion means it is evaluated and
shown like any other and holds neither the verdict nor the `satisfied` stage back.

| | required | optional |
|---|---|---|
| the verdict and its reasons | counted | not counted |
| `met` | counted | not counted; `optional` carries how many there are |
| no work serves it | `criterion_uncovered`, a failure | `optional_criterion_uncovered`, a warning |
| a recorded gap | must answer it | must answer it |

An intent whose criteria are all optional requires nothing: that is the failure
`intent_without_required_criterion`, and its verdict is `unknown`. This extends the `unknown`
of ADR 0107 — "no criterion is declared" — to "no criterion is required", for the same reason.

**A guard is an invariant that names its evidence.** `guards:` is a key of its own, a list of
records with an `id`, the `invariant` as a statement, an `evidence` kind (`test` or `claim`)
and a `ref`. `invariants:` stays a list of plain statements; a list holding both strings and
records is something the allowlist generator cannot describe, and every reader of the
statements keeps reading statements. A guard is judged by the function that judges a
criterion.

**Only a failing run violates a guard.** No issue serves a guard, so its evidence has no
declared inputs from the plan and reads stale after any unrelated change. If stale counted as
violated, every satisfied intent with a guard would turn unsatisfied on the next commit. So:

| the guard's evidence is | the guard |
|---|---|
| `failing` | is violated |
| `current` | holds |
| `stale`, `not_run`, `unresolved`, `not_derivable` | is not judged, and violates nothing |

A violated guard makes the verdict `unsatisfied` whatever the criteria say, and is named in
`verdict.guards` — a field of its own, absent when empty, so a verdict's `reasons` stay
criteria and every reader of them is undisturbed. A guard's id may not repeat a criterion's
(`duplicate_guard`), and `serves: <intent>#<guard>` resolves to nothing, as any id that is not
a criterion does.

**The stage reads the verdict.** Whether the evidence is satisfied was computed twice, once
for the verdict and once for the stage. The stage now takes it from the verdict, so an
optional criterion and a violated guard change both or neither.

**A review covers these.** The reviewed-plan revision (ADR 0112) and the binding's plan
revision (ADR 0111) take `optional` and the guards in where a record declares them, so a
stamped review of a plan stops standing when a criterion is made optional or a guard changes,
and a record that declares neither keeps the revision it had. The binding's evidence standing
takes a guard in only while it is violated: that is the change a resumed worker must be told
of.

## Consequences

- A guard that nobody runs is never violated. That is deliberate and it is a limit: a guard
  is as good as the cadence of the test it names. The recorded evidence of a CI run is what
  makes one fail.
- A violated guard does not refuse work from starting. The work that starts may be the
  repair; the verdict, the briefing and `intent explain` say that the guard is violated.
- A plain-text invariant is still judged by nothing. Writing it as a guard is how it becomes
  judged; this repository's `intent-planning` does so for "the plan engines stay identical".
- `command` and `deployment` criteria still read `not_derivable`: the ledger records runs of
  tests and claims.
- The Cockpit's intent page shows neither evaluations nor guards yet; the capabilities carry
  both.
- Completion still decides nothing about an intent (ADR 0107). What a finish may claim when
  the criteria it served are unmet is the next decision.

## Alternatives rejected

- **Stale counts as violated.** An intent with a guard would be satisfied for exactly one
  commit at a time.
- **Records inside `invariants:`.** The allowlist generator emits either a scalar pattern or
  an object walk for a list; a mixed list would leave the record keys unknown, and five
  readers take `invariants` as strings.
- **An invariant reason inside `verdict.reasons`.** The shape is pinned by tests and read by
  the drift findings as criteria; a guard's id in a field named `criterion` would be wrong
  for every one of them.
- **A rule name or policy version on the evaluation.** No such value exists in the evidence
  module; inventing one in the intent engine would add the second table rule
  `project.derived-once` forbids.
- **A violated guard refuses the binding.** The task that repairs it could not start.
