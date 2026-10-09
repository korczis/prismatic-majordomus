---
id: project.integration-follows-the-current-master
version: 3
kind: rule
title: Pull requests are integrated one at a time, each decided against the current master, never around the branch protection
description: An integration decision names the master and head it was taken against and is acted on only while a fresh observation still says the same; a merge invalidates every earlier plan; required checks, reviews and branch protection are never bypassed; a pull request is closed only on git's proof that its work is on master or on an authorised declaration whose successor landed; one executor mutates a base branch at a time; and every act is recorded.
statement: Decide integration from a fresh observation of the forge and git, name the master and head each decision was taken against, and act only while they still hold; re-plan from scratch after every merge; never merge with --admin, over a required check that has not passed on the current head, or a head that does not contain master; close a pull request only when git proves its work is on master, or when someone the repository lets declare a replacement declared one and git finds that replacement in master; hold the base branch's integration lease while mutating it; and record every act.
status: active
class: blocking
depends_on: [project.land-and-publish@1, project.accumulation-is-measured@2]
tags: [integration, git, github, safety, governance, evidence]
x-majordomus:
  tests: [test/cases/720_integration_follows_the_current_master.sh, test/cases/740_integration_is_visible_where_a_person_looks.sh, test/cases/741_a_continuous_drain_stops_cleanly_and_alone.sh, test/cases/742_only_an_outage_is_asked_again.sh, test/cases/850_integration_drains_cycle_by_cycle.sh, test/cases/852_a_dry_run_moves_nothing.sh, test/cases/855_racing_executors_merge_once.sh, test/cases/856_racing_worktrees_share_one_lease.sh, test/cases/857_required_checks_are_authoritative.sh, test/cases/858_reviews_are_authoritative.sh, test/cases/861_a_successor_that_landed_supersedes.sh, apps/majordomus-cli/tests/integration_trail.rs, apps/majordomus-cli/tests/integration_rollout.rs, test/cases/925_a_bound_check_is_only_its_apps_run.sh, test/cases/926_an_unread_writer_is_never_a_pass.sh, test/cases/927_an_unbound_base_asks_for_no_writer.sh, test/cases/928_an_authorised_supersedes_holds_then_closes.sh, test/cases/929_a_forks_declaration_decides_nothing.sh, test/cases/930_an_own_body_is_tested_on_its_author.sh, test/cases/931_a_closed_successor_releases_the_hold.sh, test/cases/932_a_squashed_successor_lands_by_its_merge_commit.sh, test/cases/933_every_page_of_cross_references_is_read.sh, test/cases/934_a_failed_declarations_read_decides_nothing.sh, test/cases/935_an_older_observation_is_refused.sh, test/cases/936_the_rule_names_every_closure_ground.sh, test/cases/937_an_unauthorised_declaration_neither_holds_nor_closes.sh, test/cases/938_a_truncated_cross_reference_read_holds.sh, test/cases/939_a_batch_landed_successor_supersedes.sh]
---
# Rationale

Seventy pull requests were open on 2026-09-30, and the repository had three competing answers
to "what can land": the forge's `mergeable` flag, which cannot run the derived-file driver and
is wrong in both directions; `scripts/land`, which merged every clean pull request onto one
branch from a list its own first merge made stale; and hand-built batches whose regressions
nobody could attribute. ADR 0101 replaces all three with one typed subsystem. This rule is
what keeps it from drifting back.

# Required behaviour

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

# Failure behaviour

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

# Verification

`bash test/run.sh 720_integration_follows_the_current_master`, the same for each case named
above,
`cargo test --test integration_trail --test integration_rollout` and
`cargo test --lib integration`.
