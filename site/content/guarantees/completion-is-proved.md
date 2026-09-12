+++
title = "The outcome completed is refused while the completion report says complete is false, naming each question of share/completion.yaml still owed and the command that settles it; the stage a task stands at is derived from the answers and never written; and every surface reads that one report"
description = "Done is one declaration, share/completion.yaml: the lifecycle stages in order, and every"
weight = 145
[extra]
claim_id = "completion-is-proved"
status = "guaranteed"
source = "docs/claims/completion-is-proved.md"
+++
{% raw %}

## What it means

Done is one declaration, `share/completion.yaml`: the lifecycle stages in order, and every
question a task must answer before it may be called finished, each naming the source its
answer is taken from — an obligation of `share/obligations.yaml`, the gate aggregate or one
gate of `.ai/repo/ci/gates.yaml`, the change set, the structural release analysis, the task
record's issue, the continuity store. `gates.policy` answers the policy; `gates.completion`
answers every question from its source, folds the answers over the stages into the stage
the task stands at, and states three facts a reader is entitled to separately: `finishable`
(nothing refuses), `verified` (every selected gate reported over this tree and none
refuses) and `complete` (every question passes or is exempt).

`majordomus check` prints the stage and what it owes. With the policy's
`verification.completed_means_complete` on, `majordomus finish --outcome completed` is
refused while `complete` is false, and the refusal is one `FAIL done <question>` line per
owed question with the command that settles it. The weaker outcomes are not refused and
record the stage they stopped at, so the next worker does not rediscover what was left.

## How it works

The stage is a fold, never a field: the first stage with a refusing question is where the
task is blocked, else the first with an unanswered one is where it stands, else it is
complete. Nothing writes a stage and no surface computes `complete` from the parts.
`majordomus evidence --run-gates` runs every gate the change selects through
`scripts/ci/run-plan` — the dispatcher CI runs — and records each exit as it finishes; an
obligation the change implies may be discharged and is declared on the task record by the
act. The HTTP route, the MCP tool, the Cockpit page, the generated section of every provider
bootstrap and the site's lifecycle page are projections of the same report and the same
policy. A question whose source the repository never declares — a gate the CI model lacks,
a version in a repository that has published no release — is `exempt` by name, which is a
different fact from silent.

## What proves it

`test/cases/280_completion_is_proved.sh` gives a disposable repository a CI model, the
planner and the dispatcher, refuses `completed` over an owed question, runs and records
the selected gates, discharges an implied obligation, and accepts `completed` exactly when
the report says complete; it also proves a weaker outcome records its stage. The crate's
own suites hold the fold (`gates::stage`) and the policy reader (`gates::policy`). The rule
is `project.completion-is-proved`; the decision is ADR 0057.
{% endraw %}
