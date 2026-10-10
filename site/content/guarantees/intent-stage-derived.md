+++
title = "An intent's stage is derived from the milestones that realise it and each criterion from the evidence ledger, and it is satisfied only while every criterion has a current passing run"
description = "An intent under .ai/repo/project/intents/ states what must become true, the invariants that must stay true, the milestones that realise it and the criteria that settle it, each criterion naming the evidence that decides it. That statement is all it stores. Its stage — declared, planned, executing, verifying, satisfied — is read from the plan, and each criterion from the evidence ledger, on every read. Finished work satisfies nothing: an intent whose milestones are all DONE is verifying until every criterion has a current passing run."
weight = 111
[extra]
claim_id = "intent-stage-derived"
status = "guaranteed"
source = "docs/claims/intent-stage-derived.md"
+++
{% raw %}

## What it means

An intent under `.ai/repo/project/intents/` states what must become true, the invariants that must stay true, the milestones that realise it and the criteria that settle it, each criterion naming the evidence that decides it. That statement is all it stores. Its stage — `declared`, `planned`, `executing`, `verifying`, `satisfied` — is read from the plan, and each criterion from the evidence ledger, on every read. Finished work satisfies nothing: an intent whose milestones are all DONE is `verifying` until every criterion has a current passing run.

## How it works

`apps/majordomus-cli/src/intent.rs` joins the intent to the derived status of each milestone it names and each criterion to the ledger's latest run of the test or claim it names. A run recorded against a source that has changed since is stale, and a failing run is not evidence; either leaves the criterion unmet. `cancelled` and `superseded` are the only stages the record itself can state.

## How to see it

```bash
majordomus intent list                   # every intent with its derived stage
majordomus intent show intent-lifecycle  # each milestone's status and each criterion's evidence state
```

The same derivation answers `GET /api/v1/intents` and the `majordomus_intents` MCP tool.

## What it does not cover

A criterion settled by a `command` or a `deployment` resolves but is never met, because the ledger records runs of tests and claims only (`intent-command-deployment-evidence`). Finishing a task does not ask whether the intent it serves is satisfied.

## Why it exists

A status written into the intent would be one more record that can disagree with the plan and the evidence it summarises. Deriving it means the intent file stays byte-identical while the work around it moves, which `test/cases/367_an_intent_is_satisfied_only_by_evidence.sh` checks at every step.
{% endraw %}
