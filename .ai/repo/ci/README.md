---
schema: context/v1
id: ai.repo.ci
kind: context
title: Continuous integration
description: The gates this repository runs, which changed paths select them, and what publication may spend.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [scripts/ci-plan, scripts/pages, .github/workflows/validate.yml]
---

# Continuous integration

The model of what CI does lives here, not in the workflow files. `gates.yaml` declares
every gate, the job that runs it, and the classes of changed paths that can affect it;
`scripts/ci-plan` reads that file and nothing else to decide what a change must run, and
`.github/workflows/validate.yml` is a thin adapter over the plan with no path list of its
own. `pages.yaml` does the same for publication: what makes the public site different,
what the fast path may spend, and how the result is measured. `github.yaml` models the GitHub projection on
the trunk: the workflow every landing starts to apply a changed plan and prove the result
(`.github/workflows/github.yml`), what that workflow may and may not do, and how long the
trunk may carry a landed change that nothing has applied yet. `plan.yaml` declares the window
of the plan's drift gate: how long the trunk may carry merged work the plan has not recorded
(`scripts/ci/plan-drift-check`, ADR 0166), and whether a refusal fails the trunk yet. `docker-image.yaml` models the deployment image: when it
is built, what its smoke proves, the immutable name it is published under, and which build may
write which cache.

`baseline.json` records what CI cost before the model existed. It is evidence for the
claim that the overhaul was worth running, not an input to any decision; nothing reads it
to plan a run.

Each file opens with the rules it encodes, because the planner enforces them and a reader
who edits the data must know them first. A path class that is wrong here is wrong once,
in the one place the tests read.

## Changing a gate

Edit `gates.yaml`, then prove the plan rather than describing it:

```bash
scripts/ci-plan --changed <path>...      # the gates that change selects, and why
bash test/run.sh 94_ci_plan              # the planner's behaviour, by mutation
```

The workflow file is regenerated from nothing — it is authored — but it may not carry a
path list, a gate name or a trigger that this directory does not declare. `docs/CI.md` is
the prose; the data is here.
