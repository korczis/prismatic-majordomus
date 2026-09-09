+++
title = "CI planned from one model, the site deployed from its verified run"
description = "The gate model names every validation gate and which classes of paths select it; the workflow is a thin adapter over that plan; the publication path proves the committed derived data current by its input hash and renders it; and the tool supervises its own repository with its own hooks."
weight = 190
[extra]
id = "ci"
status = "stable"
source = ".ai/repo/features/ci.md"
+++
{% raw %}

## What it does

`scripts/ci-plan` reads the gate model and nothing else to decide what a change must run; a
changed path no class matches escalates the whole plan and names the path. The pre-commit
hook runs `doctor`, the worktree guard and the derived-data fingerprint gate, so a stale
generated file is refused before the commit exists rather than after the merge.

## What it does not do

Publication carries only what can make the published bytes wrong; merging is decided
elsewhere. Nothing here trusts a green run it did not observe.
{% endraw %}
