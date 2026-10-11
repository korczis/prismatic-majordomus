# Majordomus Release in 15 Minutes · 2026-10-10

Goal: from `git push origin vX.Y.Z` to `curl -fsSL https://majordomus.dev/install.sh | sh`
installing vX.Y.Z takes **at most 15 minutes**, every time, with the smoke test green inside
the same bound. Owner's order, 2026-10-10 17:13Z: "release musí být max 15 minut, critical,
žádné 4h zamrznutí".

## Usage

Start in the root of the LIVE repository. Pass the prompts in order to Claude Code sessions;
`00` first, then `01` to `07` may run in parallel where the dependency line allows. Each prompt
is one milestone; each `Issue` heading is one work item, which the session allocates as an
issue record (announce the number on the board first, `project.work-is-claimed-before-it-is-built`).

## The measured baseline (2026-10-10, runs 38066863653 v0.20.0 and 38013111243 v0.19.1)

| Stage | Measured | Long pole |
|---|---|---|
| plan job | 3 min | distribution invariants 121 s, checkout 78 s |
| builds (6 targets, parallel) | 6-15 min | macos-x86_64 `package` 866 s; linux-gnu 413 s |
| publish job | 8-13 min | regenerate projections 284 s, `derive-check` re-derives 271 s, executable rebuilt 117 s |
| record PR ci (before ADR 0131) | 106 min | suite-shard (1) 86 min |
| pages-build-deployment | 1-12 min | queued 11 min for a runner on 2026-10-10 |
| smoke | 0 min run, never reached | waited on a merged record (failed by design) |

v0.20.0: tagged 16:14Z, published 16:38Z, publish job died 16:43Z (`git add` with an exclude of
the ignored `dist`), record merged by hand 17:25Z, served 17:42Z: **88 minutes**.

## The budget this pack must meet

| Stage | Budget |
|---|---|
| tag push -> plan done | 2 min |
| plan done -> slowest archive uploaded | 7 min |
| -> release published and metadata on gh-pages | 3 min |
| -> GitHub serves the new latest.json | 2 min |
| -> smoke green on all six targets | 1 min after serving |
| **total** | **15 min** |

## What already exists or is in flight (verify fresh, do not rebuild)

- I2302 (b4): release.yml stages with a command git accepts (`dist` is ignored since #855).
- I2303 / ADR 0131 (p5, branch feature/the-release-publishes-its-metadata): the release run
  publishes releases/<tag>.json and latest.json to gh-pages itself; a master deploy never takes
  the site back a release; smoke waits for the served metadata, not the merged record.
- I2296 (b4): the executor reopens a record PR opened by GITHUB_TOKEN.
- I2299 (wanted): the release chain as one typed capability with a trail and a dry run.
- I2282: the record PR's dispatched verdict does not satisfy branch protection.
- I2310 (b2, owner packs): the delivery baseline as a tracked record.

## Invariants (never traded for speed)

- The tag is cut from a commit whose `ci` passed (`scripts/ci/release-verdict`).
- Every published digest is read off the published bytes; attestations stay.
- The record still lands on master under the full required check; only the installer stops
  waiting for it.
- No `--admin`, no skipped required check, no test moved to nightly to make the number.
- The site never serves an older release than it served.

## Prompts

| # | Milestone | Depends on |
|---|---|---|
| 00 | Measure every release, and gate on the budget | — |
| 01 | The site serves a release before its record lands (finish I2303) | — |
| 02 | The publish job derives once and builds nothing | 01 |
| 03 | Every archive builds inside seven minutes | 00 |
| 04 | GitHub serves what was pushed inside two minutes | 01 |
| 05 | Smoke runs the moment the metadata is served | 01 |
| 06 | The record lands by itself | 01 |
| 07 | `release land`: one typed command, one trail | 02 05 06 |
