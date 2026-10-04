---
schema: adr/v1
id: adr-0101
kind: adr
title: Pull requests are integrated one at a time, each against a master observed a moment before
status: proposed
date: 2026-09-30
tags:
  - integration
  - git
  - github
  - governance
  - safety
provenance:
  origin: authored
related:
  - rule:project.integration-follows-the-current-master
  - rule:project.land-and-publish
  - rule:project.accumulation-is-measured
  - rule:project.no-network-no-eval
  - file:apps/majordomus-cli/src/integration/mod.rs
  - file:apps/majordomus-cli/src/integration/classify.rs
  - file:apps/majordomus-cli/src/integration/drain.rs
  - file:apps/majordomus-cli/src/capability/builtin/integration.rs
  - file:apps/majordomus-cli/src/integration/tests.rs
  - file:docs/INTEGRATION.md
  - test:test/cases/720_integration_follows_the_current_master.sh
---
# 101. Pull requests are integrated one at a time, each against a master observed a moment before

## Context

On 2026-09-30 seventy pull requests were open. None was ready by any honest definition, and
nothing said so. The repository had three answers to "what can land":

- **the forge's `mergeable` flag**, which this repository cannot use. Derived files are
  resolved by a per-clone merge driver (`merge=derived`) that GitHub cannot run, so the forge
  calls nearly every pull request conflicting over files a generator rewrites anyway;
- **`scripts/land`**, which asked git the right question and then merged every clean pull
  request onto one integration branch. That is a batch planned from a list that its own
  first merge makes stale, and a batch's regressions are nobody's in particular;
- **integration batches** built by hand (A to E), for the same reason and with the same
  cost: a red batch had to be bisected by a person.

Meanwhile the mechanics are fixed. Every merge moves the derived artifacts every other
branch carries, so every other pull request stops containing master the moment one lands.
A pull request that does not contain master cannot be merged without leaving master's
derived data stale. The required check (`ci`) takes hours.

## Decision

Pull-request integration is a typed subsystem of the executable (`crate::integration`),
with one canonical state, one classification and one executor.

1. **One canonical state.** A `PullRequestAssessment` per open pull request: what the forge
   observed, what its head is to master, its disposition, its reasons, its evidence, its
   risk, its overlaps, and the master and head it was decided against. The command line, the
   HTTP route, the MCP tool and the Cockpit render that value and nothing else.
2. **The relation to master is git's.** `git merge-tree --write-tree` with this clone's
   drivers, and `git check-attr merge` for which paths are derived, from master's own
   `.gitattributes`. A conflict on a derived path is the regeneration's, not a person's. The
   result is cached by the pair of SHAs, which are immutable. It is decided on exactly the
   head the assessment names as evaluated: a head that is not in the clone, because it moved
   during the refresh, is `unknown`, never the relation of whatever a fetched ref holds now.
   An attribute read that fails is `unknown`, never "nothing is derived". Only a pair of full
   commit ids is a cache key, and an `unknown` is never cached.
3. **Thirteen dispositions, decided in one order.** `ready` is reached only when the pull
   request targets the base, is not a draft, carries no blocking label, contains the
   current master, has its declared dependencies landed, satisfies the review policy, and
   has every *required* check passed on its current head. The required checks are the
   base's branch protection and the rulesets that apply to it, together; if either cannot be
   read, nothing is ready. A check bound to an app (`app_id` in the protection,
   `integration_id` in a ruleset) is only that app's check run: a commit status of the same
   name is not it. Of a context's reports, one still running makes it pending, and
   otherwise the newest completed report is its verdict, so a failure a re-run fixed has
   passed. A required check that is pending, missing or unreadable is not passed, and
   neither is a skip, unless the policy permits that context's skip. A base that requires
   no check proves nothing about a head: an empty set after a successful read is `unknown`
   (`no_required_checks`), never ready (owner decision D5). The review policy is read from
   the protection and rulesets the same way (approvals, code owners, stale dismissal), and
   an approval counts only on the commit it was given on: approvals of an earlier head are
   `stale`, and enough of them on the head with a code owner's still owed is
   `code_owners_pending`. The forge's review decision comes before the policy:
   `REVIEW_REQUIRED` is a pending review even where the policy requires none, because a rule
   this policy does not list can. A dependency is declared
   only by a line that opens with `Depends on`, `Stacked on`, `Requires` or `Land after` and a
   number; the same words mid-sentence are prose. A pull request is stacked only on a branch
   of this repository, never on a fork's branch of the same name.
4. **One merge at a time, and no plan survives it.** The executor's step takes no plan. It
   observes the forge, decides, observes again, and acts only if the second decision names
   the same master and head and still says `ready`. `drain` is a loop over that step. There
   is no variable in which a candidate list could survive a merge. A merge is then proved
   where it landed: the first-parent successor of the decided master must be this merge,
   with the decided head as its second parent; anything else is `unexpected_master`, even
   when the forge calls the pull request merged. The forge-side half of that guard is the
   protection's `required_status_checks.strict`; the executor reports a base without it
   and cannot set it (D8). An unverified merge holds every later drain until a person
   acknowledges it (D7); an interrupted merge is verified, never repeated, before the next
   decision.
5. **Refreshing is a separate, bounded act.** When nothing is ready, `drain --refresh` brings
   master into the first pull request that needs it: a merge commit with the derived driver
   and a fresh derive, pushed as a plain fast-forward, never a rewrite. This is what
   `project.land-and-publish` already prescribes. The pipeline is one deep: while a
   refreshed pull request waits for its checks, no other is refreshed, because merging the
   first would put the second behind again and waste its CI run. Only a run the executor
   started holds it: the required check of the head a `refreshed` event recorded as pushed,
   while it is pending or missing. Missing counts because an aggregate check is not created
   until the jobs it needs finish, but it holds only for a bound after the push (four hours),
   so a check that never reports cannot freeze every refresh. A check on a head the author
   pushed holds nothing.
6. **Cleanup demands more than merging.** Only a pull request whose head is an ancestor of
   master, or whose merge changes no file, is closed, and only with `--apply`. One that
   differs only in derived output is `possibly_redundant` and is left for a person. Age,
   shared paths and similar titles are not evidence.
7. **One executor per base branch.** An exclusive lease under the common git directory: an
   exclusive `flock` on its file, held for the executor's life, with the holder recorded
   beside it for whoever reads. The kernel decides who holds it, so executors started at
   once cannot both win, and a holder that ends, even by a crash, releases it at once; a
   live holder is never taken over, however old its record. The staleness bound is what an
   observer reports, not a licence to take the lease. Observers never take it.
8. **The network is declared, not tolerated.** `prs refresh`, `prs drain` and `prs cleanup`
   run the GitHub CLI and `git fetch`. Every other surface renders the recorded observation
   with its age. SECURITY.md names the exception.
9. **Every act is recorded.** Selections, stale decisions, merge attempts, merges with the
   master before and after, refusals, refreshes and closures go to an append-only trail that
   the `integration.events` capability serves. The trail is the repository's, not a
   checkout's: one file under the common git directory,
   `<git-common-dir>/majordomus/integration/events.jsonl`, beside the lease, so every
   worktree writes and reads the same one. Each act is appended before it is taken, and an
   act the trail cannot record is not taken.

## Consequences

Throughput is bounded by the required check: one pull request lands per CI run of its
refreshed head. That is the true cost of this repository's mechanics, and the planner makes
it visible rather than hiding it inside a batch. Parallel speculative refreshes — a merge
queue — are a later decision, and they need this one's evidence first.

`scripts/land` is deleted. `scripts/force-merge-all`, already dispositioned for deletion by
the automation inventory, is deleted with it. `scripts/ci/backlog-check --remote` measures
the backlog through `majordomus prs`.

## Alternatives rejected

- **Trust the forge's `mergeable`.** It is wrong in both directions here: measured
  CONFLICTING on clean merges and MERGEABLE on a real conflict.
- **Merge every green pull request in sequence from one list.** Every merge invalidates the
  list. The second merge would be decided against a master that no longer exists.
- **Batches.** They trade attribution for throughput, and a red batch costs a person a
  bisection. They remain a person's decision, not the executor's.
- **Merge with `--admin`.** The branch protection's refusal is the answer, not an obstacle.
- **Close pull requests by age or similarity.** Neither is evidence that the work landed.
