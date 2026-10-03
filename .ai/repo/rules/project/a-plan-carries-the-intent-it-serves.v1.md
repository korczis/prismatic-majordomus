---
id: project.a-plan-carries-the-intent-it-serves
version: 1
kind: rule
title: A plan carries the intent it serves, and is reviewed before it is executed
description: An issue names the intent criteria it exists to make true; the coverage of every criterion is derived from the plan and refused when a criterion has work nowhere or work has no purpose; and the gap a worker observed and the critique of the plan are typed records the plan is held to before execution starts.
statement: Every issue under a milestone an intent names declares the `<intent>#<criterion>` references it serves; every criterion of a live intent that has work is served by a live issue; every gap answers every criterion of its intent with an observation behind each answer that is not unknown; and no issue serving an intent is ACTIVE, VERIFY or DONE while that intent has no critique or a blocking critique finding is open. All of it is derived on read, never stored, and failures exit 10 through the intent-check gate.
status: active
class: blocking
depends_on: [project.work-serves-a-declared-intent@1, project.derived-once@1]
tags: [intent, project, planning, projections]

x-majordomus:
  tests: [test/cases/386_a_plan_is_held_to_its_intent.sh, apps/majordomus-cli/tests/intent_plan.rs]
---

# Rationale

An intent names the milestones that realise it and the evidence that settles each criterion
(`project.work-serves-a-declared-intent`). That relates an intent to milestones and to
evidence. It does not relate a *criterion* to the work meant to make it true, so an intent
could name a milestone whose issues addressed two of its four criteria, and nothing said the
other two had no work at all until verification found them unmet.

The two things planning rests on were in the same position. What a worker observed was
missing, and what a review of the plan overlooked, lived in a session's scroll: unversioned,
unenforced, and gone when the session ended. Planning therefore started from a prompt, and a
missed requirement surfaced after the work rather than before it.

Majordomus does not do the observing or the criticising. A person or a worker does, and what
they found becomes a record the repository can refuse.

# Required behaviour

- An issue may declare `serves: [<intent>#<criterion>]`. The link is authored on the issue,
  because the issue is written after the intent; an intent never lists its issues, so
  replanning does not edit an intent.
- The plan carries `serves` as data and derives no finding from it: the two plan engines stay
  identical (`test/cases/99_plan_capabilities.sh`). The intent engine judges it.
- Coverage is derived for every criterion of every live intent, and every issue reports the
  reason it exists: `intent`, `maintenance` under a milestone no intent names, or
  `unexplained` under one that realises an intent.
- A gap lives at `.ai/repo/project/gaps/<intent>.yaml`, a critique at
  `.ai/repo/project/critiques/<intent>.yaml`, both schema-valid and keyed by their intent.
- An intent with no live issue under any milestone it names is reported as unplanned, not
  refused: that is where every intent starts.

# Failure behaviour

`majordomus intent validate` exits 10, naming the finding and its subject:
`criterion_uncovered`, `issue_without_purpose`, `serves_outside_milestone`,
`serves_unknown_intent`, `serves_unknown_criterion`, `malformed_serves`,
`gap_criterion_unanswered`, `condition_without_observation`, `gap_unknown_intent`,
`gap_without_commit`, `gap_work_issue_serves_other`, `critique_unknown_class`,
`rejected_without_reason`, `planned_into_unknown_issue`, `planned_into_unrelated_issue`,
`executing_without_critique`, `executing_with_open_blocker`.

Reported and not fatal: `intent_not_planned`, `criterion_weakly_covered`, `duplicate_work`,
`milestone_contributes_nothing`, `plan_not_critiqued`, `duplicate_serves`.

# Verification

`apps/majordomus-cli/tests/intent_plan.rs` proves, through the built executable, that the
command line, HTTP and MCP answer the same coverage, that a criterion nothing serves is
refused once its intent has work, that maintenance is not a finding, and that execution is
refused before the critique. `test/cases/386_a_plan_is_held_to_its_intent.sh` drives the same
refusals over a repository `majordomus init` created, one edit at a time. The `intent-check`
gate runs `majordomus intent validate` on a change to the project model or the intent engine.
