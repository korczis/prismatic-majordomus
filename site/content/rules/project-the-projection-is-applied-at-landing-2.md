+++
title = "A landing that changes what GitHub shows applies the change itself"
description = "A landing that changes what GitHub shows applies the change itself"
weight = 173
[extra]
kind = "rule"
slug = "project-the-projection-is-applied-at-landing-2"
identity = "project.the-projection-is-applied-at-landing@2"
status = "active"
source = ".ai/repo/rules/project/the-projection-is-applied-at-landing.v2.md"
+++
{% raw %}

## Rationale

GitHub's issues and milestones are a projection of the plan on the trunk. A landing that
changes a plan record, or anything a record's rendering is derived from, leaves GitHub
behind by exactly that change until `scripts/github-sync --apply` runs from the trunk's tip.

Until 2026-10-12 that run was a person's. `project.github-projection-gated` said so in as
many words, `docs/INTEGRATION.md` gave the step to "whoever operates the executor", and
`.ai/repo/ci/github.yaml` gave it thirty minutes, after which `scripts/ci/github-check`
refuses the trunk. The design was honest about its gap: the same paragraph said a job on
every push to master would remove the manual step and called it the owner's open choice.

On 2026-10-12 the gap was paid for. A run of landings up to `81178f5362` changed I2460 to
I2478; the session operating the executor went on to the next landing; the window passed;
validate's run 38278173940 turned master red on the `github-projection` gate. Nothing in the
tree was wrong. A step that only a person performed had not been performed, and the gate
that judged it had no actor behind it. The owner's order the same day: issues, milestones
and intents are aligned with the repository, and that is enforced for the future.

This is the shape `.github/workflows/version.yml` removed for the version: a judge that
works, and a writer that runs only when somebody remembers it.

## Required behaviour

1. **The landing is the actor.** A workflow runs on every push to the default branch, with
   no path filter, and on a schedule so that a failed or cancelled run is asked again. Its
   model is [`.ai/repo/ci/github.yaml`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/ci/github.yaml); the workflow file carries no
   trigger, permission or phase the model does not declare.
2. **Ask, apply, prove, in that order.** `scripts/github-sync --check` says what is owed.
   When something is, `scripts/github-sync --apply` writes it, and `scripts/ci/github-check`
   then judges the trunk exactly as validate does. A clean first reading is its own proof
   and nothing is written.
3. **Only the trunk's tip.** The workflow has no trigger a pull request can reach, checks out
   the default branch, and asks the remote for its tip before applying. A run the trunk has
   moved past applies nothing: the landing that moved it started a run of its own.
4. **The executable or nothing.** Every phase refuses to run when `MAJORDOMUS_BIN` names no
   executable. The adapter's shell fallback read the model differently on 2026-10-12 (318
   drift findings against 26), and an apply from that reading rewrites records that have
   not moved.
5. **Least privilege.** `issues: write` and `contents: read`, declared once for the whole
   workflow, and no job widens them.
6. **One at a time, never cancelled.** One concurrency group, `cancel-in-progress: false`:
   a cancelled apply is a remote left half written.
7. **Bounded.** The job declares a bound and every step declares a smaller one, so a step
   that hangs reports itself rather than being killed with its job.
8. **A person's text is not overwritten.** The workflow never passes `--force` and never
   writes the drift baseline. A region somebody edited fails the run and waits for a person.
9. **The fallback is a fallback.** When the workflow fails, a person runs
   `scripts/github-sync --apply` from the clean tip of the default branch with the
   executable. `apply.window` bounds how long that may take before the gate refuses the
   trunk. No procedure, runbook or agent instruction names the manual apply as the step that
   normally follows a landing.
10. **A pull request's own metadata, and nothing else, from the base (version 2).** The one
   run a pull request may start is the pull request projection,
   [`.github/workflows/pr-meta.yml`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.github/workflows/pr-meta.yml), modelled by
   [`.ai/repo/ci/pr-meta.yaml`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/ci/pr-meta.yaml). It runs on `pull_request_target`,
   checks out the default branch and never the head (the head's commits and records are read
   with git as data), holds `pull-requests: write`, `issues: read` and `contents: read`, and
   writes only that pull request's labels, milestone, assignee and its managed region. It
   never creates, closes or edits an issue, so the issue projection stays the trunk's alone.

## Failure behaviour

A run that cannot read the remote, cannot build the executable, has a mutation refused by
GitHub (exit 13), or leaves the gate with a finding is a failed run of the
`github-projection` workflow, naming the step. The trunk is then behind until the next run
or a person's apply, and `scripts/ci/github-check` refuses it once `apply.window` has
passed, with the command named, as before.

`test/cases/1097_the_projection_is_applied_at_landing.sh` fails when the workflow and the
model disagree on any of the points above.

## Version 2

Version 1 said no trigger a pull request can reach, for any projection. On 2026-10-22 the
owner asked for every pull request to carry its metadata, kept current by GitHub itself
(I3970). A pull request's labels, milestone, assignee and `Refs` links are not the plan: they
are the pull request's, derived from the trunk's records, and keeping them current needs a
run the pull request starts. Version 2 admits exactly that run and keeps everything version
1 refused for the issue projection.

## Verification

`test/cases/1097_the_projection_is_applied_at_landing.sh` reads the workflow and the model
and holds one to the other: the triggers, the absence of a path filter and of any
pull-request trigger, the permissions, the concurrency group, the bounds, and the order of
the three commands. It then takes the steps' own scripts out of the workflow and runs them
against a scripted adapter and a local remote: without the executable none of them reaches
the adapter, with drift owed the tip is applied, and a checkout the trunk has moved past
applies nothing. Each assertion was broken by a mutation of the workflow or the model on
2026-10-12 and the case header lists them.

`test/cases/1038_a_head_is_judged_against_what_it_changes.sh` holds the window the fallback
is bounded by, and `test/cases/26_ci_wiring.sh` holds every workflow to having a model.
Requirement 10 is held by `test/cases/1097_the_projection_is_applied_at_landing.sh`, which
refuses a pull-request workflow that checks out the head or writes issues, and by
`test/cases/3970_a_pull_request_carries_its_metadata.sh`, which drives the projection.

What no case here proves is the run on GitHub itself: that the workflow token is accepted
for every write the adapter makes. That is observed on the first landing after this rule,
in the run's own log.
{% endraw %}
