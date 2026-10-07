+++
title = "The work realising an intent is joined across sessions, providers and handovers, and an intent whose milestones are all closed while a criterion's evidence fails is refused naming that criterion"
description = "Which work realises an intent is never written into a task, a session or a handover. It is joined on every read from the records the lifecycle already keeps: each task with every episode that worked on it and the provider of each, the handovers between them, closed session records and the peer board's claims. When the plan says every milestone of an intent is DONE and a criterion's recorded run fails, or is stale against a case that has changed, the intent is closed_work_contradicted and the realization exits 10 naming the criterion."
weight = 114
[extra]
claim_id = "intent-realization-held-to-evidence"
status = "guaranteed"
source = "docs/claims/intent-realization-held-to-evidence.md"
+++
{% raw %}

## What it means

Which work realises an intent is never written into a task, a session or a handover. It is joined on every read from the records the lifecycle already keeps: each task with every episode that worked on it and the provider of each, the handovers between them, closed session records and the peer board's claims. When the plan says every milestone of an intent is DONE and a criterion's recorded run fails, or is stale against a case that has changed, the intent is `closed_work_contradicted` and the realization exits 10 naming the criterion.

## How it works

`apps/majordomus-cli/src/intent_realization.rs` (ADR 0075) links each unit of work to an issue and says how the link is known — `declared`, `observed`, `derived` or `inferred` — keeping the strongest per issue. The `intent-realization` gate runs `majordomus intent realization` over the tracked records. `intent explain` turns the same join into sentences: the stage, each criterion and the work behind it.

## How to see it

```bash
majordomus intent realization                 # every intent, its unmet criteria, the work realising it
majordomus intent realization --intent <id>
majordomus intent explain <id>                # why it stands where it stands
```

## What it does not cover

`closed_work_unproven` (every milestone DONE, a criterion never evidenced), `criterion_closed_unmet` and live work that serves no intent are warnings, not refusals. Nothing reopens the work or returns a worker to the plan: the finding is reported and the gate fails.

## Why it exists

A closed milestone is a statement about the tracker, not about reality. `test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh` closes the work while a case fails, fixes it, breaks it again and repairs it, with the intent file byte-identical throughout, and the realization follows reality each time.
{% endraw %}
