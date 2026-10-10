+++
title = "An intent's guard — an invariant that names its evidence — is judged by the engine that judges its criteria, and a failing one keeps the intent unsatisfied whatever its criteria and milestones say"
description = "An intent says what must become true and what must stay true. The second half used to be prose: printed to the worker and judged by nothing. A guard is an invariant that names a test or a claim which would fail if it stopped being true. While that evidence is failing, the intent is not satisfied — not by met criteria, and not by finished milestones."
weight = 117
[extra]
claim_id = "intent-guard-is-judged"
status = "guaranteed"
source = "docs/claims/intent-guard-is-judged.md"
+++
{% raw %}

## What it means

An intent says what must become true and what must stay true. The second half used to be prose: printed to the worker and judged by nothing. A guard is an invariant that names a test or a claim which would fail if it stopped being true. While that evidence is failing, the intent is not satisfied — not by met criteria, and not by finished milestones.

## How it works

`guards:` on an intent record is a list of records with an `id`, the `invariant` as a statement, an `evidence` kind (`test` or `claim`) and a `ref`. The intent engine judges each with the function that judges a criterion. A guard is **violated** when its evidence is `failing`, and only then: the verdict becomes `unsatisfied` and names the guard in `verdict.guards`, and the stage is not `satisfied`. A guard whose evidence is `current` holds. One whose evidence is stale, was never run, or does not resolve is **not judged** and violates nothing.

Nothing is stored. The intent record is the same bytes before and after, and the state is derived on every read.

## How to see it

```bash
majordomus-cli intent show <id>        # each guard: violated, holds or not judged; `violated` under the verdict
majordomus-cli intent explain <id>     # one sentence per guard, with the run it was judged by
```

`test/cases/965_a_failing_guard_keeps_an_intent_unsatisfied.sh` closes an intent's work, breaks what its guard protects, records the failing run and reads `unsatisfied` at stage `verifying`; repairs it and reads `satisfied`; and makes an unrelated commit that changes nothing.

## What it does not cover

A guard nobody runs is never violated: it is as good as the cadence of the test it names. A stale guard is not a violation either, because a guard's evidence has no declared inputs from the plan and would read stale after any unrelated change. A plain-text entry under `invariants:` is still judged by nothing. A violated guard does not stop work from starting — the work that starts may be the repair — and the Cockpit's intent page does not show guards yet.

## Why it exists

An intent that reads satisfied while something it said must stay true has stopped being true is the failure the intent model exists to prevent, and the only thing standing in its way was a sentence nobody was obliged to check.
{% endraw %}
