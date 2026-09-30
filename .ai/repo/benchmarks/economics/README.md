---
schema: context/v1
id: ai.repo.benchmarks.economics
kind: context
title: Token economics benchmark
description: The methodology, suites and task corpus that measure what a session consumes with Majordomus and without it, and the raw runs recorded under them.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [apps/majordomus-cli/src/economics, test/fixtures/economics]
children:
  require_contract: false
---

# Token economics benchmark

`methodology.yaml` states the question, the measurement classes, the control and treatment,
the success gates, how runs are paired and aggregated, and the publication rule a total-token
claim must pass. `suites/` declares what is run (`live`: real harness sessions; `context`: the
context compiler, counted, no model). `tasks/<id>/` is one task of the corpus: its fixture, its
prompts, its hidden acceptance tests and its reference solution. `runs/<suite>/` holds the raw
records the runner and `economics measure` write — provider usage as reported, gate verdicts,
counted tokens — and nothing derived.

A suite, a task and a run directory are instances, not sections of the layer, so they owe no
contract of their own.

Never type a number about tokens into prose here or anywhere: it is computed by
`apps/majordomus-cli/src/economics` from these records and published through
`docs/generated/economics.*`. Changing the methodology is a version bump with its reason under
`changes`, and makes every earlier record incompatible rather than silently comparable.
