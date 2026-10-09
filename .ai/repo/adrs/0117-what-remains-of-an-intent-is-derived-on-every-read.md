---
schema: adr/v1
id: adr-0117
kind: adr
title: What remains of an intent is derived on every read, with the action it justifies, and a gap is never rewritten
status: proposed
date: 2026-10-09
tags:
  - intent
  - planning
  - evidence
  - governance
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
  - file:.ai/repo/adrs/0112-the-opposition-to-a-plan-is-executed-and-revision-bound.md
  - file:.ai/repo/adrs/0113-satisfaction-is-explained-and-held-to-its-guards.md
  - rule:project.derived-once
  - file:apps/majordomus-cli/src/intent_remains.rs
  - file:.ai/repo/project/intents/what-remains-of-an-intent.yaml
---

# 117. What remains of an intent is derived on every read, with the action it justifies, and a gap is never rewritten

## Context

After work or evidence changes, a reader asks of an intent: what still prevents it, why, and
what should be done next. The facts are derived on every read already:

- each criterion's evidence state and the verdict (ADR 0107, ADR 0113);
- each criterion's coverage — `covered`, `weak`, `observed` or `uncovered` — with the live
  issues serving it inside the intent's milestones (`intent_plan.rs`);
- the realization view's unmet criteria; and the findings `criterion_uncovered`,
  `criterion_closed_unmet` (per criterion, while the stage is `planned` or `executing`) and
  `closed_work_contradicted` / `closed_work_unproven` (once the stage is `verifying`).

No read turns them into an answer: one word per unmet criterion for what is in the way, one per
intent, and the next action each justifies.

One fact is reported by nothing. When no live issue serves a criterion that a gap marked
`satisfied` with observations, its coverage is `observed`: no work is owed and
`criterion_uncovered` is silent. When that criterion's evidence later fails, the plan still
owes no work for it, and nothing says the observation it rested on has been contradicted.

Two earlier drafts of this decision were reviewed and withdrawn. The first compared each gap
condition with the evidence; a gap is written before its work, so the success path read as
drift, and the only remedy — record a new gap — rewrites the one gap file an intent has and
moves the revision its critique is stamped with (ADR 0112). The second fixed that and was
found to define "serving" a third way, to compare readings blind to work status, and to emit
a warning from a place that cannot see the gaps.

## Decision

**One function, one serving predicate.** `intent_remains.rs` answers from two derived facts and
nothing else: the criterion's evidence state, and its coverage entry — the strength and the
live issues serving it, exactly as coverage computes them. Nothing is stored. No new command or
capability: `intent_realization.work` (`intent realization`, `/api/v1/intents/realization`,
MCP) carries the answers.

**What remains of an unmet criterion**, first match wins. "Open" is `READY`, `BLOCKED`, `ACTIVE`
or `VERIFY`; a covering issue that is not open is `DONE` (coverage leaves `CANCELLED` out).

| | condition | `remains` | `basis` |
|---|---|---|---|
| 1 | an open covering issue exists, and every open one is `BLOCKED` | `blocked` | `work_blocked` |
| 2 | an open covering issue exists, and the evidence is `failing` | `progressing` | `work_open_failing` |
| 3 | an open covering issue exists | `progressing` | `work_open` |
| 4 | the evidence is `not_derivable` | `unknown` | `not_derivable` |
| 5 | the evidence is `failing` and a covering issue exists | `failed` | `closed_work` |
| 6 | the evidence is `failing` and coverage is `observed` | `failed` | `gap_observation` |
| 7 | the evidence is `failing` | `failed` | `no_work` |
| 8 | a covering issue exists | `needs_evidence` | `closed_work` |
| 9 | coverage is `observed` | `needs_evidence` | `gap_observation` |
| 10 | the evidence is `stale` | `needs_evidence` | `passed_before` |
| 11 | otherwise | `exhausted` | `no_work` |

An unmet criterion's evidence is `stale`, `failing`, `not_run`, `not_derivable` or
`unresolved`; `current` is met and never listed. Open work is read before the evidence kind,
so a `command` criterion with an issue open on it is `progressing`, and a failing test under
open work is named (`work_open_failing`) rather than hidden. A cancelled or superseded intent
carries no `remains` and no outcome: it owes nothing, as coverage already says.

**What remains of an intent.**

| | condition | `outcome` |
|---|---|---|
| 1 | the verdict is `satisfied` | `satisfied` |
| 2 | a guard is violated | `failed` |
| 3 | a required unmet criterion is `failed` | `failed` |
| 4 | … `exhausted` | `exhausted` |
| 5 | … `needs_evidence` | `needs_evidence` |
| 6 | … `unknown` | `unknown` |
| 7 | … `progressing` | `progressing` |
| 8 | … `blocked` | `blocked` |
| 9 | no required criterion is unmet and the verdict is not satisfied (every criterion optional, or none declared) | `unknown`; the verdict says why |

`blocked` is last among the outcomes: in a plan whose issues follow one another, the later ones
are `BLOCKED` by the earlier, and an intent with any work moving is progressing. Optional
criteria carry their `remains` and decide nothing, as they decide nothing of the verdict. The
words are the ones the mission asks for (`satisfied`, `progressing`, `blocked`,
`needs evidence`, `exhausted`, `failed`); the stage, the verdict and the evidence state keep
theirs, and `docs/PLANNING.md` puts the four vocabularies in one table.

**The next justified action**, as data, never performed:

| `remains` (`basis`) | `next` |
|---|---|
| `progressing` | `work`: the open covering issues with their status |
| `blocked` | `unblock`: the blocked issues |
| `failed` (`closed_work`) | `repair`: the criterion's reproduce command |
| `failed` (`gap_observation`, `no_work`), `exhausted` | `plan_work`: an issue serving `<intent>#<criterion>` |
| `needs_evidence` | `record_evidence`: the reproduce command, then `majordomus evidence stamp` and `majordomus evidence record` on a clean tree |
| `unknown` | `none`, with why |

**What changed since a reading.** Each intent carries two values derived on the read and named
for what they hash, so neither is mistaken for the binding's task pins: `review_revision`, the
revision a review of this intent is stamped with (`intent_opposition::revision_of`), and
`remains_digest`, a digest of the outcome and of each unmet criterion's id, evidence state,
`remains` and `basis` — so a recorded run moves it, and so does a serving issue that closes. An
issue's dependencies and scope belong to the plan a review judges (`reviewed_plan`), so a change
to them is `plan_changed`. A caller asking about one intent may pass the two values it read before and is told
`unchanged`, `remains_moved` or `plan_changed` (which outranks the other). Without an intent the
input is refused: one pair cannot describe several intents. Nothing records a reading.

**One warning, for the fact nothing reports.** `Intents::build`, which already holds the gaps,
the coverage and the evaluated criteria, warns `gap_observation_contradicted` for each required
criterion whose `remains` is `failed` on `gap_observation`. It does not refuse: the change that
made a test fail is not the change that planned around it. A warning becomes an advisory
structural finding of `intent oppose` and leaves the disposition and the binding as they were;
a plan resting on a broken observation is what opposition should show.

**The gap is never rewritten.** Nothing here writes, appends to or supersedes a gap. The remedy
for a contradicted observation is an issue that serves the criterion, after which it is
`progressing`; the gap stays the observation at its commit.

## What it does not answer

- **Who may act.** Issues carry no actor and the plan no permissions; "needs authorisation" has
  no source in the canonical data and is not emitted.
- **Loops and lack of progress.** Whether a plan failed repeatedly or oscillated is a question
  over a history of readings, and none is kept; the only history is the ledger's `task.refused`
  and `task.gate` events, and reading them for this is a separate decision.
- **A pass that broke while open work serves the criterion** is `progressing` on
  `work_open_failing`, not told apart from a test written first; that needs the ledger's history
  of the test, which this does not read.

## Consequences

- The same tree answers the same on every read and every surface.
- Coverage keeps `observed` as it is; its disagreement with failing evidence is now named.
- `needs_evidence` on `closed_work` means what `criterion_closed_unmet` means; the finding stays
  the warning and `remains` is the answer, not a second warning.
- The realization view gains four fields per unmet criterion and three per intent, added beside
  the existing ones; the realization capability computes the coverage it reads with the same
  function `intent validate` uses.

## Alternatives rejected

- **Compare each gap condition with the evidence and report drift**: the success path reads as
  drift, and the only remedy rewrites the gap.
- **A new `intent reconcile` command and capability**: a second answer over facts the realization
  joins; the mission says to extend an existing capability where one exists.
- **Store each reading** to answer what changed and to detect loops: a writer on every read.
- **Make coverage drop `observed` when the evidence fails**: `criterion_uncovered` is a failure,
  and the change that broke a test would be refused for the plan's sake.
- **Reuse the binding's pins**: they hash a task's served issues and links, not one intent's
  remains; passing one where the other is expected would always answer changed.
