+++
title = "A task that names the issue or the intent it serves is briefed with what that intent asks of the work, and a handover tells the next worker when the intent moved"
description = "A worker starts from what must become true, not from an edit. A task that says which issue it executes, or which intent it serves, is handed that intent in its briefing: the statement, the invariants, the criteria this work serves with the state of their evidence, the critique of the plan and the recorded gap. Nobody copies an intent into a prompt."
weight = 118
[extra]
claim_id = "intent-in-session-context"
status = "guaranteed"
source = "docs/claims/intent-in-session-context.md"
+++
{% raw %}

## What it means

A worker starts from what must become true, not from an edit. A task that says which issue it executes, or which intent it serves, is handed that intent in its briefing: the statement, the invariants, the criteria this work serves with the state of their evidence, the critique of the plan and the recorded gap. Nobody copies an intent into a prompt.

The briefing is read from the intent engine every time it is printed. The task record keeps only what the worker named and two hashes; no stage, verdict or criterion is stored in a task, a session or a handover (ADR 0111, rule `project.an-intent-outlives-its-sessions`).

## How it works

`majordomus start "<task>" --scope <paths> --issue <id>` asks the `intents.binding` capability what the task serves before it writes anything. The answer's standing is `bound`, `maintenance`, `exempt` or `refused`; a refused answer to a question the worker put does not start. The record carries `issue` (or `intent`, or `exemption` with its reason), the standing start was told, and two pins: `plan_revision` and `evidence_standing`.

`majordomus context` asks the binding again and prints an INTENT section after PROFILE. Under a tight budget the section loses its gap conditions first and is never dropped whole. When the executable cannot be reached the section says `standing unknown` and names what was asked for, because "no intent" and "nobody could ask" are different answers.

`majordomus handover` writes what the task named and the two pins into the record's front matter. `majordomus handover --resolve` asks the binding once more and prints an `Intent:` line: `unchanged`, `evidence_moved` when a served criterion changed evidence state, `plan_changed` when the intent, a link or its critique was edited, or `unknown` when it cannot ask.

## How to see it

```bash
majordomus-cli intent binding --issue <id>        # what a task naming that issue is bound to
```

`test/cases/960_a_task_starts_bound_to_what_it_serves.sh` starts a bound task and reads its context; `test/cases/961_a_resumed_task_is_told_its_intent_moved.sh` edits a criterion, a critique and the evidence between a handover and its resolution.

## What it does not cover

A task that names nothing is briefed with no intent: under policy `intent.binding: off` nothing asks, and under `advisory` or `required` a scope-only start is resolved by its paths and recorded with its standing, but its context carries the section only when the task named an issue, an intent or an exemption. A provider episode with no task open is not bound at all (ADR 0052).

The briefing informs. It does not decide whether a criterion is met, and it refuses nothing at finish.

## Why it exists

The intent engine could already say what an issue serves, and nothing asked it: the context a worker was handed named a scope and no purpose. A briefing that depends on the worker remembering to ask is a briefing most sessions never get.
{% endraw %}
