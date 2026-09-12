---
id: project.completion-is-proved
version: 1
kind: rule
title: Completion is proved by one policy, derived from evidence, and never claimed
description: The definition of done is one shipped declaration, share/completion.yaml, whose every question names the source its answer is taken from. The completion report derives the lifecycle stage a task stands at and the one bit `complete` from those answers, every surface reads that report, and the outcome completed is refused while the bit is false. A state a worker sets, a checklist a worker ticks, or a second list of what done means anywhere is the defect.
statement: State what done means once, in share/completion.yaml; answer every question of it from the subsystem that already holds the fact; derive the stage and the bit `complete` from the answers and write neither down; read that one report from every surface; and refuse the outcome completed while the bit is false, naming each question owed and the command that settles it.
status: active
class: blocking
depends_on: [project.never-reported-is-not-green@1, project.the-version-is-measured@2, project.interfaces-are-projections@1]
tags: [completion, lifecycle, evidence, governance]

x-majordomus:
  tests: [test/cases/280_completion_is_proved.sh, test/cases/131_completion_gates.sh]
---

# Rationale

Measured on 2026-09-12 (ADR 0057): done was stated in five places — the policy's finish
contract, the obligation vocabulary, the CI model, a Rust literal of nineteen questions,
and the provider bootstraps' prose — and the completion gate could not refuse, because it
refused only over a *recorded* failing run and no run had ever been recorded. Choosing the
outcome `partial` skipped every validator. A worker reporting on its own work has no
incentive to say the validation did not run; a report that cannot refuse teaches every
worker that "done" is a word.

The cure is not a longer checklist. A checklist is exactly the artifact a worker learns to
tick, and every entry on it is a new opinion that can disagree with the subsystem that
already holds the fact. The cure is one declaration of the questions, each naming where
its answer comes from, and a report that composes the answers and derives the rest.

# Required behaviour

- `share/completion.yaml` is the only declaration of what done means. It names the stages
  in order and every question, and every question names its source: an obligation of
  `share/obligations.yaml`, the gate aggregate or one gate of `.ai/repo/ci/gates.yaml`,
  the change set, the structural release analysis, the task record's issue, or the
  continuity store. A source the policy names and the repository lacks is a reported
  problem of `gates.policy`, never a silent pass.
- `gates.completion` answers every question from its source, folds the answers over the
  stages into `stage`, and states `verified` (every selected gate reported over this tree
  and none refuses) and `complete` (every question passes or is exempt). Nothing writes a
  stage; nothing computes `complete` a second time.
- `majordomus check` prints the stage and what it owes; `majordomus finish --outcome
  completed` is refused while `complete` is false when the policy's
  `verification.completed_means_complete` is on, and each refusal names the question and
  its remediation. The other outcomes are not refused over it and record the stage they
  stopped at.
- `majordomus evidence --run-gates` runs every gate the change selects through the same
  dispatcher CI runs and records each exit; an obligation the change implies may be
  discharged and is declared on the record by doing so.
- Every surface that states done — the HTTP route, the MCP tool, the Cockpit, the
  generated section of every provider bootstrap, the documentation and the site — is a
  projection of the report or the policy.

# Failure behaviour

A second list of what done means, a stage a worker sets, a surface that computes
`complete` from the parts, or a `completed` accepted while a question of the policy is
owed is a violation. `finish` refuses with `MJ_EX_CONTRACT`, naming each owed question;
`gates.policy` reports a question whose source does not resolve; `generate --check`
refuses a bootstrap whose fragment no longer matches the policy.

# Verification

`test/cases/280_completion_is_proved.sh` refuses `completed` over an owed question, runs
and records the selected gates, discharges an implied obligation, and accepts `completed`
only when the report says `complete`; it also proves a weaker outcome records its stage.
`test/cases/131_completion_gates.sh` proves the question set is the policy's and that a
gate the model does not declare is exempt by name. The crate's own suites hold the fold
(`gates::stage`) and the policy reader (`gates::policy`).
