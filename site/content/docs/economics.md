+++
title = "Economics"
description = "the claim it refuses to make, what v0.1 controls without measuring, where the cost actually is, what the ledger alone can measure, and what honest measurement would take"
weight = 22
[extra]
source = "docs/ECONOMICS.md"
+++

{% raw %}

Short, because v0.1 measures nothing and says so.

## The claim we refuse to make

Majordomus does not reduce token spend by any percentage. No benchmark exists, so no
number appears anywhere in this repository. Every routing document studied while
designing this tool carried figures like "35 % better" with no measurement behind
them, and every one of those figures was later found to be invented.

## What v0.1 controls without measuring

The policy and profiles fix four things that, in the environments studied, were the
main sources of waste:

<div class="overflow-x-auto">

| Lever | Waste it addresses | How v0.1 handles it |
|---|---|---|
| always-loaded budget | every session re-reading a 1,000-line contract | hard line cap, `doctor` fails over it |
| profile effort | maximum reasoning as the default | effort is a profile field; escalation is a recorded event, not a habit |
| profile verbosity | narrating every intermediate step | verbosity and presentation are profile fields, separate from effort |
| scope and finish contract | duplicated and abandoned work | one task per checkout, overlap reported, `finish` refuses unverified completion |

</div>


These are declared controls. Their effect is not measured by v0.1.

## Where the cost actually is

Token cost per task is a sum, and most optimisation effort goes to its smallest term:

```
total tokens per task
  = context loaded per session
  + the same context loaded again in the next session
  + reasoning
  + output
  + retries and rework
  + a second worker doing the same investigation
```

"Write shorter answers" trims the fourth term. A worker that first read a repository's
worth of context it did not need has already spent more than every answer it will write.
Majordomus targets the terms in this order, by expected size: context per session (profile
toggles, the always-loaded budget), repeated context (durable state and handovers instead
of transcripts), retries (a finish contract that refuses before a human has to), duplicated
parallel work (scope claims and overlap reports), reasoning profile discipline, and output
verbosity last.

No percentage is attached to any of this. None has been measured, so none is claimed; the
sections above and below say what measuring would take. A team that already runs short
sessions, selects context deliberately, hands over well and scopes work explicitly will
save little, and for them the value is standardisation, reproducibility and auditability
rather than cost.

## What can be measured today, from the ledger alone

Without any provider telemetry, `state/ledger.jsonl` already yields per task:

<div class="overflow-x-auto">

| metric | from |
|---|---|
| sessions per task | `task.started`, `task.handed_over`, `task.finished` sharing a `task_id` |
| wall-clock time | `started_at` on the record to the `task.finished` timestamp |
| handover overhead | handover files naming the task |
| workers involved | distinct `owner` values across a task's records |
| verification runs | the `verify` object on `task.finished` |

</div>


Not yet: refused finishes (a refusal writes nothing, by contract; recording it is a
contract change under review), and anything counted in tokens.

## What would have to be true to measure

The unit that matters is **cost per accepted outcome**, not tokens. To compute it,
each `task.finished` ledger record would need, from the provider:

- input, output, and cached tokens for the session
- model and effort actually used
- elapsed time and tool-call count

and, from the person: whether the outcome was accepted, and whether rework followed.

The ledger already records the task, profile, outcome, contract result, and
verification command with its exit code and duration. The provider-side fields are the
missing half. v0.3 in the roadmap adds them only for providers that expose them
honestly; any `estimated_` field stays out of enforcement and out of comparisons.

## The comparison this would enable

The unit is tokens, cost and time **per accepted outcome**, never per prompt: a debugging
task run under the right profile may spend more on one prompt and less on the task, because
it needed two sessions instead of three and no retry. Once measured, the question is not
"which model uses fewer tokens" but which profile produces the best accepted-outcome rate
per unit cost for a task class. That
comparison, across a team's real work, is the durable value. Nothing in v0.1 pretends
to have it.
{% endraw %}
