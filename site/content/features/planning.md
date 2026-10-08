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
only with the evidence its contract requires. Above the branch the same edge is read in
both directions and stored nowhere: the `trace` capability module derives an issue's
branches and commits from git, `scripts/traceability` joins the pull requests GitHub holds,
and a commit no execution contract accounts for is reported as unattributed rather than
quietly left out.

Above the milestones, an intent states what must become true and the evidence that settles
it. `majordomus intent` derives its stage from the plan and each criterion from the evidence
ledger, answers which intent the work on an issue serves when a worker asks, refuses a
criterion no issue serves and work that started before its plan was critiqued, and joins the
work realising it across sessions and providers; the Cockpit shows the same answers at
`/cockpit/intents`.

## What it does not do

It does not talk to GitHub on its own: the projection of issues and milestones is rendered
offline and applied only when a person runs the sync with a token. Nothing here estimates
effort or schedules dates. An intent is enforced when work starts only where the
policy requires binding (`intent.binding: required`); elsewhere it is judged after the fact.
The GitHub projection closes a milestone without reading the intents it realises.
{% endraw %}
