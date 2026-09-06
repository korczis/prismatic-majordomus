---
schema: context/v1
id: ai.repo.benchmarks.pages
kind: context
title: Publication baselines
description: What publishing the site cost on a named platform, kept as the comparison for the next run.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [scripts/pages]
---

# Publication baselines

`baseline.<platform>.json` records what a publication run spent, phase by phase, on that
platform. `scripts/pages` writes it and compares against it; the budgets it is judged
by are in `../../ci/pages.yaml`, not here, because a budget is a decision and a baseline
is an observation.

Only repository-controlled latency is budgeted. Queue time, runner allocation and
GitHub's own deployment step are recorded in these files and never gated: they are not
this repository's seconds to spend.
