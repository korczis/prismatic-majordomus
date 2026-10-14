+++
title = "Where the policy requires binding, starting an issue or a task that serves an intent with no critique, or with a blocking finding open, is refused when it is started rather than reported afterwards"
description = "The review of a plan is a precondition of the work, not a finding about it. In a repository whose policy says intent.binding: required, an issue whose intent was never critiqued, or whose critique has a blocking finding still open, does not become ACTIVE, and a task that would execute it does not start."
weight = 120
[extra]
claim_id = "intent-refused-at-plan-start"
status = "guaranteed"
source = "docs/claims/intent-refused-at-plan-start.md"
+++
{% raw %}

## What it means

The review of a plan is a precondition of the work, not a finding about it. In a repository whose policy says `intent.binding: required`, an issue whose intent was never critiqued, or whose critique has a blocking finding still open, does not become `ACTIVE`, and a task that would execute it does not start.

## How it works

`majordomus plan start <id>`, the `plan.transition` capability (HTTP and MCP) and `majordomus start --issue <id>` each ask `intents.binding` before they write. The binding judges the issue's links with the same coverage `intent validate` reports from and refuses with a cause: `intent_not_critiqued`, `open_blocking_finding`, or a broken link. Both plan engines print the same sentence, `<id> may not start: <refusal>`, and neither writes a stamp or a ledger line.

An answer that cannot be read refuses as well: a required gate that passed when it could not ask would not be a gate.

## How to see it

```bash
majordomus-cli intent binding --issue <id>        # the standing, and each refusal with its cause
```

`test/cases/962_plan_start_asks_the_binding_in_both_engines.sh` moves issues of an uncritiqued intent through both engines with the policy key absent and then required, and compares the two refusals word for word. `test/cases/960_a_task_starts_bound_to_what_it_serves.sh` does the same for `majordomus start`.

## What it does not cover

It is a policy choice. With `intent.binding` absent or `off`, `plan start` succeeds exactly as before and `majordomus intent validate` names `executing_without_critique` or `executing_with_open_blocker` afterwards; under `advisory` a task's refused binding is reported and the task starts. This repository's own policy is `advisory`.

A critique is judged by its presence and its open blocking findings. Whether it reviewed the plan as it now stands is not judged: the record carries no revision of the plan it reviewed.

## Why it exists

`intent validate` found work on an unreviewed plan only once the work had begun, and gating `majordomus start` alone would have left `plan start` as a door beside the gate.
{% endraw %}
