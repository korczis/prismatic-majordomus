---
id: project.completion-never-decides-satisfaction
version: 1
kind: rule
title: Completion discharges obligations and never decides whether an intent is satisfied
description: Whether a task is finished and whether an intent is satisfied are separate questions with separate authorities; a finished task, a closed milestone or a completion policy never writes, stores or decides an intent's satisfaction, which only the intent engine derives from evidence.
statement: Only the intent engine decides whether a satisfaction criterion is met, from the evidence ledger and the claim join; completion policy and `majordomus finish` may read an intent's derived standing but never write an intent record, store a stage or a verdict, or declare a question whose answer is that an intent is satisfied, so a completed finish leaves every intent's stage and criterion states as the plan and the evidence derive them.
status: active
class: blocking
depends_on: [project.work-serves-a-declared-intent@1, project.derived-once@1]
tags: [intent, completion, evidence, governance]

x-majordomus:
  tests: [test/cases/895_a_finished_task_does_not_satisfy_an_intent.sh]
---

# Rationale

A task is finished when it discharged the obligations its profile and policy declare. An intent
is satisfied when something is observably true for the people it is for. The two answers move
independently: a finished task can leave its intent false, and an intent can become true
without any task finishing. If completion decided satisfaction, a finished task would be one
policy edit away from stamping an intent satisfied — the merge-stamping ADR 0070 rejected,
under another name. ADR 0107 records the decision and supersedes ADR 0070's plan to make
satisfaction a completion-policy question.

# Required behaviour

- A criterion's verdict and an intent's satisfaction are computed by the intent engine
  (`apps/majordomus-cli/src/intent.rs`) over the evidence ledger and the claim join, and by
  nothing else.
- Completion policy (`share/completion.yaml` where it exists) declares no question whose answer
  is that an intent is satisfied. A question may read an intent's derived standing as an input.
- `majordomus finish`, with any outcome, writes no file under `.ai/repo/project/intents/` and
  stores no intent stage or verdict anywhere.
- After a finish with outcome `completed`, `majordomus intent show` reports the same stage and
  criterion states it reported before the finish, for every intent.

# Failure behaviour

A change that lets completion decide satisfaction fails
`test/cases/895_a_finished_task_does_not_satisfy_an_intent.sh`, which names the moved stage, the
moved criterion state or the changed intent record.

# Verification

`test/cases/895_a_finished_task_does_not_satisfy_an_intent.sh` proves through the built
executable that a completed finish next to a DONE milestone with an unmet criterion exits 0,
leaves the stage `verifying` and the criterion `not_run`, and leaves the intent record and the
intents directory unchanged; a passing run then recorded is what satisfies the intent.
