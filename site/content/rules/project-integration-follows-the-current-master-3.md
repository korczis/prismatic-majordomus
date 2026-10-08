+++
title = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
description = "Pull requests are integrated one at a time, each decided against the current master, never around the branch protection"
weight = 106
[extra]
kind = "rule"
slug = "project-integration-follows-the-current-master-3"
identity = "project.integration-follows-the-current-master@3"
status = "active"
source = ".ai/repo/rules/project/integration-follows-the-current-master.v3.md"
+++
{% raw %}
## Rationale

Seventy pull requests were open on 2026-09-30, and the repository had three competing answers
to "what can land": the forge's `mergeable` flag, which cannot run the derived-file driver and
is wrong in both directions; `scripts/land`, which merged every clean pull request onto one
branch from a list its own first merge made stale; and hand-built batches whose regressions
nobody could attribute. ADR 0101 replaces all three with one typed subsystem. This rule is
what keeps it from drifting back.

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

`tests/integration_trail.rs` fails when an act is taken without being recorded first, and
`tests/integration_rollout.rs` when continuous mode is reachable without the record. The
module's unit tests (`cargo test --lib integration`) hold the dispositions, who may declare
a supersession, the stale-decision refusal, the re-plan after every merge and the cleanup
threshold.

## Verification

`bash test/run.sh 720_integration_follows_the_current_master`, the same for each case named
above,
`cargo test --test integration_trail --test integration_rollout` and
`cargo test --lib integration`.
{% endraw %}
