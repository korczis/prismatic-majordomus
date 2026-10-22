---
schema: adr/v1
id: adr-0162
kind: adr
title: A release is promoted from a built candidate and served within ten minutes of its tag
status: proposed
date: 2026-10-14
tags:
  - release
  - distribution
  - pages
  - ci
  - throughput
related:
  - file:.ai/repo/adrs/0131-a-release-publishes-its-metadata-before-its-record-lands.md
  - file:.ai/repo/adrs/0019-distribution-is-one-model-and-every-surface-that-ships-the-t.md
  - file:.ai/repo/adrs/0006-ci-planned-from-one-model-and-the-site-deployed-from-its-verified-run.md
  - file:.ai/repo/adrs/0037-a-gate-whose-runner-is-scarce-is-planned-on-demand-so-that-the-verdict-arrives.md
  - file:.ai/repo/adrs/0134-the-version-is-allocated-at-landing.md
  - file:.ai/repo/adrs/0155-an-expedited-landing-is-the-owners-word.md
  - file:.github/workflows/release.yml
  - file:.github/workflows/validate.yml
  - file:.github/workflows/pages.yml
  - file:scripts/site-deploy
  - file:docs/DISTRIBUTION.md
provenance:
  origin: authored
---

# 162. A release is promoted from a built candidate and served within ten minutes of its tag

## Context

ADR 0131 set out to serve a release within 15 minutes of its tag, and said so plainly: "The
15 minutes the owner set are not reached by this decision alone; they need the build and the
publish job shortened as well, which is not decided here." This decision is that remainder.

On 2026-10-14 the owner raised the bar. The words, as written to the sessions:

- "tohle se musi poresit systemove, pres github, cancelovat actions, lepe cacheovat, lepe
  resit deploy, potrebujeme se dostat na release time per patch, minor versi pod 5-10 minut"
  (this has to be solved systemically, through GitHub: cancel actions, cache better, handle
  deploys better; we need to get release time per patch or minor version under 5-10 minutes).
- Asked whether to write the amendment of ADR 0131 that the measurement showed was needed:
  "ano" (yes).

Measured on 2026-10-14 over the last five releases, from tag to `releases/latest.json` on
majordomus.dev naming the tag. The figures come from the run and job timestamps of the
release, validate and pages workflows. The report is in the evidence of the pull request that
carries this decision.

| release | plan | builds end (critical job) | publish | tag to served |
|---|---|---|---|---|
| v0.26.0 | 4.7 | +31.4 (macOS x86_64 26.4) | 13.8, after 5.8 queued | 61.9, with Pages queued 10.2 |
| v0.25.0 | 4.4 | +31.3 (linux aarch64 musl queued 16.8) | 12.9 | 50.6 |
| v0.24.1 | 3.7 | +24.0 (macOS x86_64 20.0) | 12.5 | 41.0 |
| v0.23.0 | 4.1 | +25.9 (macOS x86_64 21.6) | 12.6 | 39.2 |
| v0.22.0 | 3.4 | +28.9 (macOS x86_64 25.2) | 13.2, after 13.6 queued | 61.3 |

All figures are in minutes. Where the time goes:

- **Building.** Every release compiles the crate fourteen times, all cold. A cache saved
  under a tag can be read only by that tag, and no trunk job saves the release keys. The
  macOS x86_64 leg spends 9.1 minutes on a debug build whose only job is to ask the model for
  the artifact's name, then 14.9 minutes on the release build, on the slowest runner class.
- **Publishing.** The publish job checks out the full history (75-153 s against 5 s
  shallow). It recompiles the executable the build jobs already built (2.2 min), re-derives
  the whole tree (4 min, 3.3 of them site data), and judges the derivation by running it
  again (3.7 min). Only then does it publish two files.
- **Deploying.** ADR 0131's metadata deploy pushes to `gh-pages`, and GitHub's own
  `pages-build-deployment` serves it. That job runs for about 40 s, but it queued for 0-47
  minutes in the runs sampled, because it competes for the same twenty hosted job slots as
  everything else.
- **Waiting.** A validate run costs 360-530 job-minutes across 15-19 jobs. Twenty slots give
  1,200 job-minutes an hour, so one trunk run and one or two pull request runs saturate the
  pool. The six smoke jobs, two of them on macOS, hold slots for 3-9 minutes each while they
  poll for the metadata.

## Decision

**1. A release is promoted from a candidate built on the trunk, not built at tag time.**
Every push to master that changes what ships builds the release matrix the model generates
(ADR 0019). Because the version is allocated at landing (ADR 0134), every such commit is a
release candidate. The candidate archives are kept as artifacts of that run, named by commit
and target. The tag's run downloads the candidate archives of the tagged commit and runs
`release-verify` on them exactly as it runs it on archives it built. It refuses an archive
whose recorded commit, target or digest does not match. When a target has no candidate, it
builds that target as it does today. The matrix, the names and the record still come from
the model; nothing is composed by hand.

**2. The release metadata is published on a fast check, and the full derivation runs beside
it.** This amends ADR 0131 decision 1. The publish job writes the record, generates its
projections with `majordomus generate --check` scoped to the release projections (seconds,
not minutes), and publishes `releases/<tag>.json` and `releases/latest.json`. The whole
derivation and `scripts/derive-check` move to a parallel record job, which proposes the
record's pull request. ADR 0131 decisions 2-5 stand unchanged: two files and nothing else,
the refusals, never going back a release, and the record landing under the full verdict.
The publish job uses the executable the build or candidate produced instead of compiling its
own, and checks out without file contents it does not read (`filter: blob:none`).

**3. The site is deployed by the run that built it, not by a branch build GitHub queues.**
This amends the deploy mechanism of ADR 0131 decision 1 and of the pages workflow. The Pages
source becomes GitHub Actions. `scripts/site-deploy` uploads the site artifact, and the same
job deploys it with the Pages deployment action. The trunk deploy and the release metadata
deploy are two modes of the one script, as ADR 0131 required. The release metadata mode
composes the two files onto the artifact of master's last deploy, so ADR 0131 decision 4,
that the site never goes back a release, holds by construction. Publish no longer dispatches
`pages.yml`. ADR 0037's timeout inequality applies to the deploy step.

**4. Release work has priority over pull request validation, and nothing is cancelled for
it.** While a release run is open, the plan job of a pull request's validate run waits,
bounded to 30 minutes, before fanning out, so one waiting job replaces fifteen to nineteen.
The trunk's runs, release runs and dispatches never wait. The cancel-nothing rule of the
validate workflow (cases 26 and 3019) stands. I2710 carries this clause.

**5. Smoke tests do not hold slots while they wait.** One job waits until the served
metadata names the release, and the smoke legs start only after it has.

**6. The bound is measured on every release.** `release-check` records tag-to-served for
each release as a derived measurement, with the time of every stage. A release served more
than 10 minutes after its tag is a finding on the board that names its slowest stage.

## Alternatives rejected

**Skip targets for a patch release.** This contradicts ADR 0019: a release that is missing
a supported target cannot be recorded. Once candidates exist, decision 1 makes it save
almost nothing anyway.

**Fold the record's verdict into the release run.** Then the pipeline approves its own pull
request. ADR 0131 decision 5 requires the full verdict. Decision 2 takes the record off the
path to production instead.

**Cancel pull request runs to free slots at tag time.** This contradicts the cancel-nothing
rule (cases 26 and 3019), and another session's run on a current head is not stale (ADR
0155). Decision 4 gets the same slots by waiting before fan-out, so nothing is lost.

**Self-hosted runners on the mesh nodes.** They would add slots and warm caches, but the
repository is public, and a pull request's code would run on the owner's machines. That needs
a decision of its own about isolation, and this decision does not depend on it.

**Keep the full derivation in front of publication.** Its guarantee does not move. The
derivation check still judges the record's tree before the record lands. What moves is only
that installation no longer waits for it, which is the step ADR 0131 already took for the
record's verdict.

## Consequences

- From tag to served, the path becomes about one minute of plan, two of promotion, three and
  a half to four of publication and deploy, and one of the metadata check: about 8-9
  minutes with the pool uncontended. Decision 4 keeps it near that during the day.
  - Without decision 2 it is about 15 minutes.
  - Without decision 3, the Pages queue is added: typically about 4 minutes, at worst 47.
- **Under 5 minutes** is not decided here. It needs plan, promotion and publication in one
  job, archives packed by the candidate run, the tag cut only on a green `ci`, and a
  guaranteed job slot at tag time. On twenty hosted slots the last means capping how many
  jobs a pull request run fans out to.
- **Trunk cost.** Each push to master costs about 40-60 more job-minutes for the candidate
  build. A candidate that is never tagged expires with its run's artifacts.
- **Changes that need no decision.** The low-risk changes the measurement found are made
  under existing rules:
  - the debug build in `release-package` goes, and the artifact name is taken from the
    matrix;
  - macOS x86_64 is cross-compiled on the arm64 runner by the model, with the Intel smoke leg
    kept as native proof;
  - checkouts are shallow or blob-less;
  - caches are saved only on master and keyed per target, with one rolling key for the image
    build.
- **Issues.** Each one is a slice that lands on its own.
  - I2725: a release is promoted from a trunk candidate.
  - I2726: metadata is published on a fast check, and the derivation runs in a parallel
    record job.
  - I2727: Pages is deployed by the run, from GitHub Actions.
  - I2728: smoke tests wait in one job.
  - I2729: tag-to-served is measured and bounded.
  - I2710: release priority.
  - I2711 and I2712: the pages job waits for its deployment, and the release-check pending
    window.
- **Self-update.** A release served sooner reaches every mesh node sooner (ADR 0161). The two
  decisions together are what the owner's "deploy asap, start using it automatically" asks
  for.
- **Status.** This decision is `proposed`. The owner ordered the goal and the amendment in the
  words quoted above, and accepting the text is the owner's act. If the owner rejects it, ADR
  0131 stands as written.
