+++
title = "A task that declares obligations reaches the outcome completed only when each one has evidence recorded against it, and evidence taken over files that have since changed, or at a commit the branch has since left, no longer discharges anything"
description = "A task may declare requires beside scope. Scope is a containment promise — where a"
weight = 145
[extra]
claim_id = "obligation-closure"
status = "guaranteed"
source = "docs/claims/obligation-closure.md"
+++
{% raw %}

## What it means

A task may declare `requires` beside `scope`. Scope is a containment promise — where a
worker may write. `requires` is a delivery promise — what the worker owes before the
outcome `completed` is available. The tokens are declared in `share/obligations.yaml`:
implementation, tests, docs, generated, rules, commit, push, target, pages, deploy, verify.

`majordomus evidence --covers <token>` records that one of them has been discharged, naming
the command or artifact that produced it — narrative is not evidence — and the hash of the
tracked files the obligation names. Four tokens are never recorded: `commit`, `push`,
`target` and `pages` name facts the tool establishes for itself, and
[obligation-establishment](obligation-establishment.md) is that half of the story. `check` and `finish` evaluate every declared obligation
on every run through the doctrine `majordomus.obligation-closure`. An obligation with no
evidence fails. An obligation whose recorded input hash no longer matches the tree fails,
with both hashes named. An obligation whose fact is remote — a push, an integration, a
publication, a deployment — is bound to the commit it was taken at instead, and fails once
the branch has left it.

Only an outcome of `completed` is refused. A worker reporting `partial` or `blocked` is
being honest, and refusing that would teach them to claim `completed` instead.

## How it works

`mj_validate_obligations` in `lib/evidence.sh` reads the task's `requires`, then for each
token reads the most recent `task.evidence` line for that task from the ledger. Evidence is
a ledger line rather than a new store, because the ledger is already append-only, ordered
and integrity-checked, and its envelope already carries the head, the branch and the
session.

Currency is decided by recomputation, never by a timestamp. `mj_obligation_inputs_hash`
expands the obligation's pathspecs with `git ls-files` and hashes them through
`mj_inputs_hash` in `lib/common.sh` — the same implementation, moved there from the site
generator, that produces the site's own `source_hash`. So "is this evidence still true?"
and "is the published site still current?" are the same question asked of different inputs.
For a remote obligation there are no inputs and `mj_git_label` decides, with the four values
the rest of the repository already uses: `exact`, `advanced`, `diverged`,
`different_context`. No fifth vocabulary was invented.

One reporter, `mj_obl_verdict`, decides whether a shortfall refuses or merely reports, so
the judgement is written once and `check` can name a stale evidence without blocking work.

## How to see it

```bash
majordomus start "a change" --scope lib/
# declare what it owes, then try to finish without proving it
majordomus finish --outcome completed --note "done"
# FAIL obligation  tests — owed, and no evidence was recorded; exit 10

majordomus evidence --covers tests --type test --command 'bash test/run.sh'
# evidence: tests recorded for t-... (inputs 28a9ccc503fb)
majordomus check     # OK obligation tests — discharged over inputs 28a9ccc503fb

echo change >> lib/a && git add -A && git commit -qm change
majordomus finish --outcome completed --note "done"
# FAIL obligation  tests — the evidence was taken over inputs 28a9ccc503fb and this tree
#                  hashes to 8c27a0fd8ec7; it no longer describes what it proved
```

## What it does not cover

It does not judge whether the command a worker names actually proves the obligation: a task
may record `--command true` against `tests` and the ledger will hold it. What it removes is
the ability for that record to survive the change it was supposed to describe. It does not
run anything itself, and it does not invent obligations — a task that declares none behaves
exactly as it did before, which is what every task in this repository does today.

Of the remote tokens, `deploy` and `verify` still depend on facts a laptop cannot establish
alone; their evidence names a commit and a time, and the honest report for work that has not
reached the trunk is that the obligation is not discharged.

## Why it exists

The finish contract already refused, but everything it refused over was inside the working
tree. A worker could satisfy every line with the work uncommitted, unpushed, absent from the
trunk, unpublished and unverified, and the record would say completed. Implemented,
committed, pushed, integrated and deployed are five different facts.

The second half is worse and quieter. Evidence that cannot expire is a claim about the past
presented as a claim about the present. A test result recorded before a change says nothing
about the tree that exists after it, and a report that treats the two as the same is how
"done" comes to mean nothing in particular.
{% endraw %}
