# RELEASE 15 · 04 GitHub serves what was pushed inside two minutes

## Operating contract
As in 00. The deploy path is one (`site-deploy-one-path`): extend scripts/site-deploy and
pages.yml, do not add another.

## Objective
From the push of the release metadata to gh-pages until majordomus.dev serves it: at most 2
minutes. On 2026-10-10 GitHub's `pages-build-deployment` sat queued for a runner from 17:29:45Z
to about 17:41Z, behind this repository's own `validate` jobs.

## Issue 1: Measure the queue
For the last 30 Pages builds, record queued and run seconds and what else was running
(`gh run list` of the repository at that moment). Attribute the queue.
Acceptance: the table, with run ids.

## Issue 2: The release does not wait behind ci it made moot
During a release, runs whose verdict is moot occupy runners: the run of a record PR that has
merged, the run of a superseded master commit. Give the executor (or the release capability)
a typed, audited action that cancels exactly those, under the integration lease, trail first,
never a run whose verdict anything still waits for.
Acceptance: case with a scripted forge: a merged PR's run is cancelled, a live PR's run is not.

## Issue 3: Decide the Pages build type with evidence
Branch-source (legacy) Pages runs GitHub's build job on the repository's runner pool. A workflow
deploy (`actions/upload-pages-artifact` + `actions/deploy-pages`) runs in the deploy job itself.
Measure both on a scratch Pages site (or a fork); switch only if it is faster and keeps the
custom domain, build.json and `pages-live` semantics.
Acceptance: measurement; a decision recorded as an ADR either way.

## Completion report
Queue seconds before/after; the decision and its evidence.
