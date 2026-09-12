+++
title = "A commit plan carries a fingerprint of the repository, the worktree, HEAD and the exact change set it was derived from, so a plan acted on after the tree moved is refused with the reason rather than applied"
description = "A plan over the working tree is derived from a tree at a moment. In a repository worked by"
weight = 186
[extra]
claim_id = "commit-plan-is-refusable"
status = "guaranteed"
source = "docs/claims/commit-plan-is-refusable.md"
+++
{% raw %}

## What it means

A plan over the working tree is derived from a tree at a moment. In a repository worked by
one person at a time that is a detail. Here it is the correctness question: several workers
share a checkout, more share a repository, and a plan computed at one HEAD and executed at
another commits files somebody else staged, under a message about work that is no longer what
changed.

The claim is that a plan says what it was made of, precisely enough that acting on it later
is refusable.

## How it works

`PlanFingerprint` carries four values:

- **repository** — git's common directory, which every worktree of one repository shares and
  no two repositories do;
- **worktree** — this checkout's own canonical path, which is what keeps two worktrees of one
  repository from sharing a plan;
- **head** — the commit the plan was made at, or `unborn`;
- **changes** — SHA-256 over every change with its stage and its status, in order. Restaging
  one file moves it; touching a file the plan does not contain does not.

`differs_from` returns a sentence rather than a boolean, because *"the plan is stale"* is not
something anybody can act on and *"HEAD moved from a1b2c3d4e to e4f5a6b70 since the plan was
made"* is. It is not a lock and it does not prevent the race; it refuses to be the one that
loses it silently.

The same value carries a second property that has nothing to do with concurrency. A change
set holding generated files is planned as **one** commit, not several, and the rationale names
`project.derived-files-regenerated` as the rule that decided: a generated file is a projection
of the whole tree, so only the last of several commits could carry a current one and every
earlier commit would be stale by construction — which the `derived-current` hook then refuses,
one commit at a time, after the split has already been made. The planner found this on its
first real run, against its own change set, where it proposed putting twenty-three generated
files in a commit of their own.

## How to see it

```bash
majordomus commit plan                       # the plan, with the fingerprint and each group's reason
majordomus commit plan --format json | jq .fingerprint
git add <one more file>
majordomus commit plan --format json | jq .fingerprint    # the change hash moved
bash test/run.sh 274_commit_policy           # the case proves all of it against real git
```

The case stages a file and reads the fingerprint, stages another and asserts the hash moved,
commits and asserts HEAD moved, then adds a second linked worktree of the same repository and
asserts that the two plans share a `repository` and differ in `worktree`.

## What it does not cover

It detects, it does not prevent. Two workers can still plan against the same tree and race to
execute; what cannot happen is that the loser's plan applies silently to a tree it was not
made for.

It fingerprints the change set, not the content. A file edited and re-edited to the same
`status` letters between two plans produces the same hash — the index's own staleness, not
this subsystem's, and `git status` reports the same thing.

Nothing consumes a persisted plan yet, because nothing persists one: `commit.plan` is a pure
function of the tree and answers fresh each time. The fingerprint exists so that a caller that
*does* hold one — a Cockpit page open for ten minutes, an agent that planned before running
tests — can ask whether it still describes the tree in front of it.

## Why it exists

This repository has had as many as ten sessions working in it at once, across a primary
checkout and thirty-odd linked worktrees. The failure this guards against is not hypothetical:
a commit made from a plan whose tree has moved is indistinguishable, afterwards, from a commit
somebody meant to make.
{% endraw %}
