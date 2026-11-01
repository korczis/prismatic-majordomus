+++
title = "Bring a branch current by the cheapest safe strategy"
description = "Decide, from the integration engine's own assessment, whether a branch needs nothing, master merged in, a repair by its owner, a fresh branch from master or to wait, and carry out only that, with every rewrite preceded by a rescue ref and every push a fast-forward."
weight = 8
[extra]
id = "rebaser"
status = "active"
version = 1
source = ".ai/repo/skills/rebaser/SKILL.md"
+++
{% raw %}

## Purpose

A branch falls behind master many times a day here, and most of the time the only conflict
is over `merge=derived` files that a regeneration settles. The expensive mistakes are the
opposite ones: rewriting a branch that did not need it, which throws away the verdict CI
spent four hours on, or "resolving" an authored conflict by taking one side.

So this skill never starts from "rebase". It starts from what the integration engine
(`majordomus prs`, ADR 0101, `docs/INTEGRATION.md`) has already decided about the branch,
picks the strategy that disposition allows and that costs least, and does nothing else.
The strategy is a consequence of the evidence; the skill is named for the job, not for the
git command.

It is not a lander: it never moves master (that is `integrator`, under the landing seat),
and it never decides a semantic conflict.

## When to use

- A pull request reads `needs_refresh`, `conflicting` or `needs_repair` in `prs status`.
- Before handing a head to the landing seat, to know it contains master.
- A train link turned behind after a lower link landed.
- A branch's own checkout is stale against master and you are about to continue work in it.

Not this skill: landing (`integrator`); closing what is already on master (`prs cleanup`,
or `assess-before-deleting` for local refs and worktrees).

## Procedure

### 1. Read the assessment, do not compute it

```
majordomus prs refresh            # the one read that reaches the network
majordomus prs explain <n>        # disposition, relation to master, evidence, rank
majordomus prs trains             # when <n> is a link of a stack
```

An observation older than the last landing is not evidence about the current master: refresh
first. The disposition and the relation are the inputs of step 2; never re-derive them by
hand with `git rebase` dry runs.

### 2. Choose the strategy from the assessment

<div class="overflow-x-auto" tabindex="0">

| The assessment says | Strategy | Act |
|---|---|---|
| relation `up_to_date` or `carries_master`; `ready`; `waiting_for_checks` | **nothing** | none — a head that contains master keeps its verdict |
| `redundant`, `superseded`, relation `contained`, `patch_ids_upstream` | **nothing here** | cleanup's lane: `majordomus prs cleanup` lists it, `--apply` closes it |
| `needs_refresh`, relation `behind` (conflicts, if any, only on `merge=derived` paths) | **merge master in** | `majordomus prs repair <n>` reads; `majordomus prs repair <n> --apply` acts, or let `prs drain --refresh` take the queue's first |
| `conflicting` (an authored path conflicts) | **repair by the owner** | the owner merges master into its own branch in its canonical worktree, resolves each authored file with evidence (tests of both sides' intent), derives, commits with hooks, pushes a fast-forward |
| `waiting_for_dependency`, `other_base`, link above an unlanded link | **wait** | land the lower link first; a stacked head is refreshed only after its base lands |
| `blocked`, `draft`, `unsafe`, `waiting_for_review`, `unknown` | **wait** | the table's own next step in `docs/INTEGRATION.md` (a label, a review, `prs refresh`) |
| unpublished local commits only (no remote ref, no pull request) | **rebase** allowed | `git rebase origin/master` in the branch's own worktree — nothing anyone else holds is rewritten |
| a published branch whose history must change (a member's fix that cannot be merged, a split) | **fresh branch from master** | rescue ref first (step 4), then a new branch from origin/master carrying the commits, and a new pull request that declares it supersedes the old one |

</div>


Two rules decide every row:

- **A published branch is never rewritten in place.** Bringing master in is a merge commit
  pushed as a fast-forward (`project.land-and-publish`, `never force-push`); a forced push is
  not a strategy here.
- **A head that already contains master is never touched.** Every new commit starts the
  required check again (about four hours at the 2026-11-01 median), so a needless refresh
  costs more than any conflict it avoids.

### 3. Respect who owns the branch

Read the peer board before acting on a branch you did not create
(`majordomus mesh peers --full`, `scripts/collision-check --new <paths>`). A branch another
session claims is theirs to merge into; tell them the strategy instead. The landing seat
merges master only into train tops and repair lanes it holds (`project.the-cooperation-model-is-the-owners-word`).
Work in the branch's canonical worktree (`majordomus worktree`, `docs/WORKTREES.md`), or in
a scratch worktree as `prs repair --apply` does — never in somebody else's checkout.

### 4. Before any rewrite, a rescue ref you have proven

```
git branch rescue/<branch>-<yyyymmdd> <old head>
git push origin refs/heads/rescue/<branch>-<yyyymmdd>
git ls-remote origin refs/heads/rescue/<branch>-<yyyymmdd>   # must print the old head
```

The convention for a stale checkout brought current is `refs/rescue/` (see
`project.worktree-topology`); either way, the ref is proven on the remote in the same
step that is about to rewrite, as `assess-before-deleting` requires.

### 5. Act, then read the queue again

After a push, the head has a new required-check run and every assessment above it is stale.
`majordomus prs refresh` and `majordomus prs explain <n>` again: the expected answer is
`waiting_for_checks`. Anything else is the next round of step 2, not a retry of the same act.

A refresh that has to happen twice for the same pull request because master moved under it
is starvation, not bad luck: `prs status` lists a pull request passed over three times as
`starving`. Bring it to the landing seat as a train member rather than refreshing it a third
time.

## Output

One row per candidate:

<div class="overflow-x-auto" tabindex="0">

| pull request / branch | disposition, relation | strategy | act and its exit | rescue ref (proven) | queue afterwards |
|---|---|---|---|---|---|

</div>


followed by the heads pushed with their check runs, and every candidate left alone with the
reason. Report each row with the states of `report-verification-state`: a pushed refresh
whose check has not reported is PARTIAL, never done.
{% endraw %}
