---
schema: adr/v1
id: adr-0073
kind: adr
title: An issue names the intent criterion it serves, and the coverage of every criterion is derived from the plan
status: proposed
date: 2026-09-15
tags:
  - intent
  - project
  - planning
related:
  - rule:project.derived-once
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:apps/majordomus-cli/src/intent_plan.rs
  - file:apps/majordomus-cli/src/plan.rs
  - file:share/schemas/majordomus/issue/issue.v1.schema.json
provenance:
  origin: authored
---

# 73. An issue names the intent criterion it serves, and the coverage of every criterion is derived from the plan

## Context

ADR 0070 makes an intent a typed record: a statement, invariants, the milestones that realise
it and satisfaction criteria, each naming the evidence that settles it. It relates an intent to
milestones and to evidence. It does not relate a criterion to the work that is meant to make it
true. So an intent can name a milestone whose issues address two of its four criteria, and
nothing says that the other two have no work at all until somebody notices, at verification,
that they cannot be met.

The questions a planner and a reviewer ask before execution are therefore still unanswerable
from the repository: which issue carries this criterion, why does this issue exist, which
criterion has no work, which milestone contributes nothing to the intent that names it, and
which two issues are doing the same thing.

## Decision

An issue may declare `serves`: a list of `<intent-id>#<criterion-id>` references. The link is
authored on the issue, because the issue is the execution contract and is written after the
intent; the intent does not list its issues, so re-planning never edits an intent.

The join is derived, never stored, by `intent_plan::coverage` over one `IntentOutline` per
intent (its id, criterion ids, milestones, and whether it is retired) and the derived `Plan`.
The intent engine converts its own record into an outline, so intent files are read one way.

An intent nobody has planned yet is not the same defect as a plan that misses a criterion: the
first is where every intent starts. An intent whose milestones carry no live issue is reported
once as `intent_not_planned`, a warning, and its criteria are not each refused for want of work.
Once any live issue sits under one of its milestones, a criterion the plan misses is a failure.

Coverage refuses, as failures:

- `criterion_uncovered` — no live issue serves a criterion of a live intent that has work;
- `issue_without_purpose` — a live issue under a milestone an intent names serves nothing;
- `serves_outside_milestone` — an issue serves a criterion of an intent that does not name
  the issue's milestone;
- `serves_unknown_intent`, `serves_unknown_criterion`, `malformed_serves` — dangling or
  malformed links.

It warns on `criterion_weakly_covered` (every serving issue requires no evidence),
`milestone_contributes_nothing` (a named milestone carries none of the intent's criteria),
`duplicate_work` (two live issues serve one criterion over overlapping scope) and
`duplicate_serves`.

An issue under a milestone no intent names has origin `maintenance`. That is legitimate work
stated as such, never a finding, and never assigned an invented intent. A cancelled or
superseded intent owes no coverage.

Planning starts from two typed records a worker writes and Majordomus validates, never from a
prompt. Majordomus does not reason about the gap or the plan; a person or a model does, and
what it found becomes refusable:

- a **gap** (`.ai/repo/project/gaps/<intent>.yaml`, `majordomus.gap/v1`) records, at an
  `observed_at` commit, observations with their source, and one condition per criterion —
  `satisfied`, `missing`, `conflicting` or `unknown` — with the observations behind it, plus
  risks and suggested work. A gap that leaves a criterion unanswered, answers a criterion the
  intent does not declare, or asserts a state with no observation is refused; `unknown` is
  allowed and never read as satisfied. A criterion observed `satisfied` with declared
  observations asks for no planned work (coverage strength `observed`); whether it is *met*
  remains the evidence's question (ADR 0070).
- a **critique** (`.ai/repo/project/critiques/<intent>.yaml`, `majordomus.critique/v1`)
  records the adversarial pass over the plan: findings classed by the question asked
  (`missed_requirement`, `unproven_assumption`, `insufficient_work`, `unnecessary_work`,
  `regression_risk`, `surface_missing`, `delivery_verification`), each `blocking` or not, and
  resolved `open`, `planned` into an existing issue that serves the intent, or `rejected` with
  a reason. An issue serving an intent that is ACTIVE, VERIFY or DONE while the intent has no
  critique (`executing_without_critique`) or has an open blocking finding
  (`executing_with_open_blocker`) is refused; planned work with no critique yet is a warning.

The plan carries `serves` as data only. It adds no finding of its own, so the two plan engines
(`lib/project.awk` and `plan.rs`, held identical by case 99) are unchanged in what they derive.

## Alternatives rejected

**The intent lists the issues per criterion.** Every re-plan would edit the intent, and the
intent would become a second index of the plan.

**Coverage inside `plan validate`.** The plan would have to read intents, and the awk engine
would need a second implementation of a join that only the Rust engine can serve on every
surface.

**Uncovered criteria as warnings.** A criterion nothing will make true is the defect this
exists to catch before execution; a warning is read after the work is done. The one warning
kept, `intent_not_planned`, is about an intent with no plan at all, which no failure could
describe without refusing every newly declared intent.

## Consequences

- Once the intent kind lands (ADR 0070), `intent validate` reports these findings beside its
  own, and the `intent-check` gate refuses an uncovered criterion.
- An intent declared before its issues are written reads `intent_not_planned` until they are;
  declaring the intent and the issues that serve it in one change is what clears it.
- Nothing generates a plan from a gap. A worker turns suggested work into issues that
  `serves` the criteria, and records `work[].issue`; the validation refuses a link that does
  not hold. Automatic derivation stays out of scope, as ADR 0070 says.
- Staleness of a gap against later commits is not judged here: `observed_at` is recorded so a
  later slice can compare it with the paths the intent's criteria reference.
