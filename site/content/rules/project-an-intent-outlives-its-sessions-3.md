+++
title = "An intent outlives the sessions and providers that realise it, and closed work does not outrank evidence"
description = "An intent outlives the sessions and providers that realise it, and closed work does not outrank evidence"
weight = 72
[extra]
kind = "rule"
slug = "project-an-intent-outlives-its-sessions-3"
identity = "project.an-intent-outlives-its-sessions@3"
status = "active"
source = ".ai/repo/rules/project/an-intent-outlives-its-sessions.v3.md"
+++
{% raw %}

## Rationale

An intent says what must become true and the plan says which issues realise it, but the work
itself happens in episodes: a Claude Code window starts a task, hands it over, and a Codex
window finishes it. If the answer to "what is this session realising" lived in the session, it
would end with the session; if it lived in a field an agent fills in, it would be as good as the
agent's memory. And if "the work is closed" were allowed to stand for "the intent is true", a
regression after closure would never be seen.

Every fact the lineage needs is already recorded by something that does not depend on the
worker remembering it: the ledger stamps each task, handover and plan transition with its
episode, the episode's start line names its provider, a closed session record lists the issues
it moved, and a peer claim names its scope. The join over them is the lineage, and it is only
honest if a link guessed from overlapping paths is never shown as a link a person declared.

## Required behaviour

- `intent_realization.work` (`majordomus intent realization`, `GET /api/v1/intents/realization`,
  `majordomus_intent_realization`) joins every ledger task, closed session record and peer claim
  to the intents it realises, through issue and milestone, and writes nothing.
- Each link carries `via` and `provenance`: `declared` when the work cites the issue, `observed`
  when its episode moved the issue, `derived` when its branch names the issue, `inferred` only
  when nothing stronger exists and an open issue's scope overlaps. One link per issue, the
  strongest.
- A task record and a handover may carry what the worker named at `start` — `issue`, `intent`,
  `exemption` with its reason — and two pins, `plan_revision` and `evidence_standing`, which are
  hashes `intents.binding` answered and a later read compares (ADR 0111). They carry no stage,
  no verdict, no criterion and no evidence state: a name is a declaration and a pin is a
  question to ask again, and neither is the answer. The issue a task named is a `declared`
  link, as an issue cited in its title is.
- `session.started` carries `provider` and `provider_session` when a provider opened the
  episode, so a closed episode keeps its provider.
- Each intent reports its unmet criteria with the issues serving each, the work and providers
  realising it, and its drift: `closed_work_contradicted`, `closed_work_unproven`,
  `criterion_closed_unmet`, and — where the stage and the verdict disagree (ADR 0107), once per
  intent — `evidence_ahead_of_plan` (the verdict is `satisfied` while the stage is `planned` or
  `executing`) and `closed_work_not_satisfied` (every milestone DONE and the verdict
  `unsatisfied` or `unknown`, naming each criterion that holds it back with its evidence kind and
  state). Live work serving no intent is the warning `work_serves_no_intent`.
- `intent_realization.explain` (`majordomus intent explain <id>`) states why the intent stands
  where it stands, sentence by sentence, from the same derivations.
- The `intent-realization` gate runs `majordomus intent realization`, which exits 10 on any
  `closed_work_contradicted`.

## Failure behaviour

`majordomus intent realization` prints each finding with its level, code, subject, message and
reproduce command and exits 10 when an intent whose milestones are all DONE has a criterion whose
recorded run is failing or stale (`closed_work_contradicted`). Every other drift —
`closed_work_unproven`, `criterion_closed_unmet`, `evidence_ahead_of_plan`,
`closed_work_not_satisfied` — and `work_serves_no_intent` is a warning: printed with its code and
reproduce command, never an exit 10. An intent that is not there is refused by name, never
answered empty.

## Verification

`test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh` drives the loop
through the lifecycle: a Claude Code episode through its own session hook and a Codex episode
through the session entry point carry one task across a handover; closing every issue and the
milestone while one case fails leaves the intent `verifying` with exit 10; the fix satisfies it;
breaking the behaviour takes the satisfaction away again; the repair restores it; and the intent
file is byte-identical throughout. `apps/majordomus-cli/tests/intent_realization.rs` proves the
provenance of each kind of link and that the command line, HTTP and MCP answer the same join.
`test/cases/367_an_intent_is_satisfied_only_by_evidence.sh` proves `evidence_ahead_of_plan` while
the milestone is open and its absence once it closes;
`test/cases/895_a_finished_task_does_not_satisfy_an_intent.sh` proves `closed_work_not_satisfied`
naming the criterion after a completed finish.
`test/cases/960_a_task_starts_bound_to_what_it_serves.sh` proves that a bound task's record holds
the names and the pins and no line of the intent itself, and
`test/cases/961_a_resumed_task_is_told_its_intent_moved.sh` that a handover holds the same and
that a pin a body tries to write is refused.
{% endraw %}
