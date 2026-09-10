---
schema: adr/v1
id: adr-0037
kind: adr
title: A gate whose runner the repository cannot get on demand is planned on demand, so that the verdict arrives
status: accepted
date: 2026-09-10
tags:
  - ci
related:
  - file:.ai/repo/adrs/0006-ci-planned-from-one-model-and-the-site-deployed-from-its-verified-run.md
  - file:.ai/repo/ci/gates.yaml
  - file:.github/workflows/validate.yml
  - file:.github/workflows/pages.yml
  - file:scripts/ci-plan
  - file:docs/CI.md
  - file:test/cases/94_ci_plan.sh
  - file:test/cases/26_ci_wiring.sh
provenance:
  origin: authored
---

# 37. A gate whose runner the repository cannot get on demand is planned on demand, so that the verdict arrives

## Context

ADR 6 gave this repository one required status: `ci`, a job that always runs, needs every
other job, and is green only when every gate the plan selected succeeded. The design is
right and it stopped working, for a reason no gate could report.

Measured on 2026-09-10. Four validation runs of master — `67ec7c2aa`, `c08c44733`,
`31285bdf5`, `beaee8654` — were sitting in `queued`, the oldest for seven hours. Inside them
every Linux job had finished; in the run of `31285bdf5` five of them had *failed*. Three
jobs had never started: `bench`, `macos` and `install (macos-latest)`, all on
`macos-latest`. `ci` needs all three, so `ci` had not run, so nothing reported at all — and
a check that never reported and a check that reported green are the same absence in the
interface a person looks at. Master carried five defects through a night that way.

Waiting was not going to fix it. This repository pushes to master about every seven minutes
on an integration day; the macOS suite runs for about 106 minutes; each run asks for three
macOS jobs, and pull-request runs ask for them too, because the classes that select `macos`
are the crate, the shell tool, the distribution and the pipeline — almost every change.
Demand of that shape is not served by a shared macOS pool at any queue depth, and the gates
were therefore not protecting anything: over that whole day, not one of them completed.

Separately and with the same shape: the Pages deploy job budgeted five minutes over work
that includes waiting for GitHub's own Pages build. That build took 33, 121, 204 and 338
seconds over the day's four deployments; the job was killed mid-wait at 13:56:12 with the
publication already done at 13:52:20, and GitHub reports a job killed by its timeout as
`cancelled`. A successful publication read as a failed one.

## Decision

- **The model marks the gate, not the workflow.** A gate in `.ai/repo/ci/gates.yaml` may
  carry `on-demand: true`. `scripts/ci-plan` leaves such a gate out of every plan that did
  not ask for it — the full plan included — and records, per gate, that it was not planned
  and why; `--on-demand` asks for them. `implies` and `requires` cannot bring one in past a
  plan that did not ask. The field is true or absent, and a gate cannot be both `always` and
  `on-demand`.
- **Three gates carry it, and what they share is a runner, not a subject**: `macos`,
  `rust-bench` and `installer-live` (whose job is a matrix with a macOS leg, and
  `needs.<job>.result` is one value over both legs).
- **The events that can afford them ask for them.** The nightly schedule, a
  `workflow_dispatch` and a pull request labelled `ci:full` pass `--on-demand`; a routine
  push and an ordinary pull request do not. The schedule moves from weekly to nightly, so
  the cadence of the macOS gates is a day rather than never.
- **`ci` is unchanged.** It still needs every job the model names and still turns red when
  one of them is red; on a routine push those three are *skipped* rather than queued, which
  is what lets it report. Requiring it in the branch rule stays the repository owner's
  setting.
- **A timeout bounds one kind of second, and which timeout fires is arithmetic.** In
  `pages.yml` the controlled steps carry their own minutes — that is what turns a hang into
  a failure — and the job's bound is set strictly greater than the sum of them. That
  inequality is the decision, not the numbers: a step killed by its own bound is a failed
  step, and on the publication probe, which is `continue-on-error`, a *reported* one; a job
  killed by the job's bound is `cancelled` over every step at once, the succeeded ones
  included. A job bound at or below the sum makes the job's bound the one that fires, which
  is what reported a finished deployment as a failure. The publication probe waits ten
  minutes rather than four, inside a step bounded at eleven so that the script's own
  diagnostic — which commit the site is still serving — survives; the job is bounded at
  twenty-five over a controlled budget `.ai/repo/ci/pages.yaml` puts at sixty-five seconds,
  because the rest of that number is GitHub's Pages build, which that file already says is
  measured and never budgeted. `test/cases/26_ci_wiring.sh` holds the inequality.

## Consequences

- A verdict exists on every push and every pull request, and it can be required.
- A macOS-only regression can reach master and be found by the nightly run rather than at
  the merge. What does *not* move is the per-push evidence about the deployment: `pages-live`
  runs on Linux, is not on demand, and still reads the published site on every push; only the
  installer half of that question moves to the night, with the macOS leg it shares a job with. That is the trade this record exists to name. It is a smaller loss than it
  reads as: before this change the macOS gates were not completing at all, so nothing was
  caught at the merge either. A reviewer who expects platform-dependent trouble — the shell
  tool, the distribution, the crate's process and file handling — labels the pull request
  `ci:full` and gets them before merging.
- The nightly run is the one that carries them, so somebody has to read it. A nightly that
  goes red and is not read is this defect again, one cadence slower.
- `scripts/ci-plan --full` no longer means "every gate". It says so in its reason line, and
  `just ci-full` passes `--on-demand`, because a person asking for everything means
  everything.
- ADR 6's "Triggers and concurrency" (a weekly schedule) and its "Platforms" bullet (macOS
  in every full plan) are amended by this record rather than superseded: the rest of that
  decision stands.

## Alternatives rejected

- **A second, Linux-only verdict beside `ci`.** It would arrive, but it leaves the macOS
  jobs queueing on every push, so the gates stay unreported and the queue stays saturated
  for every other run; and two statuses whose names differ by a word is how the wrong one
  gets required.
- **Fewer macOS jobs per push** (folding the benchmark check into the macOS job). It halves
  a number that has to fall by two orders of magnitude. The arithmetic above does not care.
- **Raising the timeouts and waiting.** The jobs were queued for seven hours against a
  110-minute bound; no bound this repository writes changes what a shared runner pool
  supplies.
- **Deleting the macOS gates.** They catch a real recurring class here — bash 3.2 and BSD
  userland — and this record moves their cadence, not their content.
- **Buying the capacity.** GitHub's larger and paid macOS runners are not queued the way the
  free pool is, and they would restore the macOS gates to every push without changing a line
  of the model. That is the alternative this record does not get to reject: it is the
  repository owner's, it costs money rather than design, and it is the right answer if the
  trade below turns out to hurt. Should it be taken, the change here is one field — remove
  `on-demand` from the three gates — and this record is superseded rather than worked around.
