+++
title = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
description = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
weight = 106
[extra]
kind = "rule"
slug = "project-integration-follows-the-current-master-4"
identity = "project.integration-follows-the-current-master@4"
status = "active"
source = ".ai/repo/rules/project/integration-follows-the-current-master.v4.md"
+++
{% raw %}
## Rationale

Seventy pull requests were open on 2026-09-30, and the repository had three competing answers
to "what can land": the forge's `mergeable` flag, which cannot run the derived-file driver and
is wrong in both directions; `scripts/land`, which merged every clean pull request onto one
branch from a list its own first merge made stale; and hand-built batches whose regressions
nobody could attribute. ADR 0101 replaces all three with one typed subsystem. This rule is
what keeps it from drifting back.

The third came back within a week. On 2026-10-07 and 2026-10-08 five batches were built by
hand, because one landing per ninety-minute run could not keep up with twelve open pull
requests; four of the five carried fixes written on the batch branch, and none recorded who
rode. ADR 0114 does not forbid the batch. It makes it the executor's act, with a record and a
shape a bisection can use, and it is the hand-built one that stays replaced: version 4 of
this rule adds what holds that.

## Required behaviour

- A pull request is `ready` only when it targets the base, is not a draft, carries no blocking
  label, contains the current master, has its declared dependencies landed, satisfies the
  review policy, and has every required check passed on its current head. Pending, missing,
  skipped and unreadable are not passed, and a check the base binds to an app is passed only
  by that app's own run: another writer's run of the name is not it, and a run whose writer
  was not read is unknown.
- The executor observes the forge before deciding and again before acting, and merges only
  when both decisions name the same master and head. It holds no plan across a merge.
- No integration code passes `--admin`, force-pushes, or rewrites a branch. Bringing master
  into a branch is a merge commit pushed as a plain fast-forward.
- Closure needs an explicit `--apply` and one of four grounds: the head is an ancestor of
  master; the merge changes no file; every commit the head has and master lacks is on master
  as an equal patch; or a successor was declared for it by an owner, member or collaborator
  in a pull request of this repository — in its own body or in the successor's — and git
  finds that successor's head or merge commit in master. The forge's word that a pull
  request merged proves nothing here. A successor the forge does not call merged counts
  only when its head lives in this repository: a fork's head, closed unmerged, proves
  nothing wherever it points, and neither does a successor that changes no file.
- An authorised declaration holds the pull request it replaces while the successor is open,
  and releases it when the successor is closed without landing. A declaration from anyone
  else, or from a fork, is reported as `possible_supersession` and neither holds nor closes.
- A pull request whose cross-references were not read whole is held until they are: an
  unread declaration is never taken for none, and the rest of the queue goes on. A
  declarations read the forge will not answer is not a hold: the refresh fails and the
  observation recorded before it is not replaced. A pull request that differs only in
  derived output is left for a person.
- Only what a body's author states is a declaration of replacement: a marker on a quoted
  line, in a fenced code block or in an HTML comment declares nothing.
- Mutations hold the base branch's integration lease. Observers do not.
- Every selection, stale decision, merge, refusal, refresh and closure is appended to the
  audit trail before it is taken, to one trail per repository under the common git
  directory; an act the trail cannot record is not taken.
- The lease is an exclusive `flock` held for the executor's life and taken for the base it
  observed. A live holder is never taken over; a holder that ends, even by a crash, releases
  it at once. A lease that cannot be renewed ends the run rather than acting without it.
- A merge is complete only when it is proved where it landed: master contains the decided
  master, and the merge commit's parents are that master and the decided head on master's
  first-parent line. A merge that cannot be proved stops every drain until a person runs
  `prs drain --resume-after-failure`.
- A failure is recorded with its class. A candidate's own failure (stale, conflict, a new
  failing check, a revoked review, a policy refusal) holds that candidate back and the drain
  goes on; an unverified merge or an unreadable forge stops it. Only an outage is asked
  again, a bounded number of times; a merge is never retried.
- A continuous drain runs alone, refreshes before every action, waits a bounded interval
  between cycles and stops cleanly on a signal. It is the last stage of the rollout and is
  refused until the trail holds `ROLLOUT_MERGES_BEFORE_CONTINUOUS` verified merges since
  the last merge that could not be verified (ADR 0101 §13).
- Several pull requests land in one merge only as a batch, and a batch is composed only by
  the executor (`prs compose --apply`, ADR 0114): on a person's request, under the base
  branch's integration lease, from a fresh observation, with `compose_selected` and
  `compose_attempted` on the trail before anything reaches the remote. Without `--apply`
  composing is a read. A batch is then one pull request like any other; nothing merges it
  but the drain.
- A pull request is a member only when it is open on the base, not a draft, without a
  blocking label, with its head in this repository, no auto-merge armed and no declared
  successor; the review policy is satisfied; every required check passed on its current
  head; and every dependency it declares has landed or is a member placed before it. Its
  head may be behind master: that is the one gate a member may fail. It is not itself a
  batch, and no open batch already names it. The size is the policy's
  `integration.batch.max_members`, which `--max` only lowers; fewer than two is not a batch.
- A batch carries a manifest, `.ai/repo/integration/batches/<id>.yaml` of kind
  `integration-batch/v1`, written by the act and never by hand: the master it was composed
  on and each member in order with its number, its head and the merge commit that carries
  it. Its pull request's body declares it the successor of every member.
- On its first-parent line from that master a batch branch holds one merge commit per
  member, whose second parent is the member's recorded head, and then at most one
  composition commit, which changes only the manifest, the version files `release bump`
  writes and `merge=derived` paths. A fix is written on the member's branch and the batch
  is composed again.
- `prs batch-check`, a gate of every plan, refuses a branch that merges the current heads
  of two or more other open pull requests, or that adds or changes a manifest, unless it is
  exactly the batch its one manifest says. When it needs the open pull requests and cannot
  read them it cannot run, and that is never a pass.
- `prs compose --apply` is refused while the layer does not hold ADR 0114 as `accepted`,
  which is a person's act, and until the trail holds a verified merge since the last merge
  that could not be verified.

## Failure behaviour

`test/cases/720_integration_follows_the_current_master.sh` fails when the integration code
names `--admin` or a force push, when a read-only `prs` command reaches the network or
writes the audit trail, when the relation to master is taken from the forge's `mergeable`,
or when the executor loses its re-observation before acting. Case 850 fails when a drain
does not re-plan after a merge, 852 when a dry run moves anything, 855 and 856 when two
executors both merge, 857 and 858 when a required check or review is taken from what is
visible rather than what is required, 925 when another writer's run passes a check bound to
an app, 926 when a run whose writer was not read passes one, 927 when a base that binds
nothing has its writers asked for, 742 when anything but an outage is asked again, and 741
when a continuous drain runs without its record, beside another executor, or past a signal.

Supersession and closure: 861 and 928 fail when an authorised successor's landing is not
seen or its hold is lost, 929, 930 and 937 when a declaration nobody entitled made holds or
closes anything, 931 when a closed successor still holds, 932 and 939 when a landing git can
see is not counted, 933, 934 and 938 when a declarations read that was partial or failed
releases or closes, 935 when an older observation is read as this one, and 936 when this
rule stops naming a ground the code closes on.

Batches: `tests/integration_compose.rs` fails when a composition is taken outside the lease
or before its trail lines, when a member rides with a pending or failing required check, a
missing review, a fork's head or an unlanded dependency, when the pushed branch is not one
merge per member and one commit, when the manifest or the `Supersedes` lines do not name
the members in order, when a second composition pushes over the first, or when `--apply`
is taken while ADR 0114 is not accepted or before the trail's record. Case 1000 fails when
the gate passes a hand-built batch, a batch with an authored commit, a manifest that
disagrees with the merges or names a member that moved, or reports clean when the forge
could not be read. Case 1001 fails when a dry run records, fetches or asks the forge for
anything, when it does not name who is left out and why, or when `--apply` is reachable
while the decision is still proposed.

`tests/integration_trail.rs` fails when an act is taken without being recorded first, and
`tests/integration_rollout.rs` when continuous mode is reachable without the record. The
module's unit tests (`cargo test --lib integration`) hold the dispositions, who may declare
a supersession, the stale-decision refusal, the re-plan after every merge and the cleanup
threshold.

## Verification

`bash test/run.sh 720_integration_follows_the_current_master`, the same for each case named
above,
`cargo test --test integration_trail --test integration_rollout --test integration_compose`
and `cargo test --lib integration`.
{% endraw %}
