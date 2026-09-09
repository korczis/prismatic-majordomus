+++
title = "Milestones and issues as data; status derived, never stored"
description = "Milestones are outcome specifications and issues are execution contracts, one YAML file each; the dependency graph decides execution waves and the next ready issue; a milestone whose dependencies are not accepted is blocked whatever its own issues say; and the GitHub projection is rendered offline from the same model."
weight = 130
[extra]
id = "planning"
status = "stable"
source = ".ai/repo/features/planning.md"
+++
{% raw %}

## What it does

`majordomus plan validate` refuses a cycle, an edge that resolves to nothing and a record
that breaks its schema; `plan next` answers which issues may be executed now; `plan roadmap`
draws the milestone graph the website renders. A branch created for an issue is named after
it, so the worktree topology can read the issue back from the branch, and an issue is done
only with the evidence its contract requires.

## What it does not do

It does not talk to GitHub on its own: the projection of issues and milestones is rendered
offline and applied only when a person runs the sync with a token. Nothing here estimates
effort or schedules dates.
{% endraw %}
