+++
title = "An intent whose criterion names evidence that resolves to nothing is refused by name"
description = "Every reference an intent makes has to land on something the repository holds: each milestone in the plan, each governance entry among the rules and decisions, and each criterion's evidence — a test file, a claim of docs/CLAIMS.yaml, a deployment object. A reference to nothing is a failure named after the intent, not an intent that silently stays unmet."
weight = 112
[extra]
claim_id = "intent-links-resolve"
status = "guaranteed"
source = "docs/claims/intent-links-resolve.md"
+++
{% raw %}

## What it means

Every reference an intent makes has to land on something the repository holds: each milestone in the plan, each governance entry among the rules and decisions, and each criterion's evidence — a test file, a claim of `docs/CLAIMS.yaml`, a deployment object. A reference to nothing is a failure named after the intent, not an intent that silently stays unmet.

## How it works

`majordomus intent validate` resolves every link and exits 10 on `unresolved_evidence_ref`, `unknown_milestone` or `unresolved_governance`, and on an intent with no milestone or no criterion. The `intent-check` gate in `.ai/repo/ci/gates.yaml` runs it, so a pull request that breaks a link is refused before it merges.

## How to see it

```bash
majordomus intent validate                 # every finding; exit 10 when any is a failure
majordomus intent validate --format json
```

## What it does not cover

It checks that a reference resolves, not that the test it names asserts what the criterion says. Whether the evidence is current is the stage derivation's question (`intent-stage-derived`).

## Why it exists

An intent whose criterion points at a renamed test would read as planned forever: never met, never failing. Refusing the dangling link turns that silence into a finding with a name.
{% endraw %}
