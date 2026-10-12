# RELEASE 15 · 00 Measure every release, and gate on the budget

## Operating contract
You work on the LIVE repository `korczis/prismatic-majordomus`. Read AGENTS.md, CLAUDE.md, `.ai/**`,
docs/DISTRIBUTION.md, docs/CI.md, `.ai/repo/ci/release.yaml`, `.github/workflows/release.yml`,
`.github/workflows/pages.yml`. Run `majordomus context`, read the peer board and the mesh
(`majordomus_peers`, `majordomus_mesh_peers`), announce before you write. Never build in the
primary checkout; work in `majordomus worktree create <branch>`.

## Objective
A release's latency is a measured, recorded, gated quantity: every release records how long each
stage took, against the budget the owner set, and a release over budget is a named finding with
the stage that spent the time.

## Issue 1: The release budget is declared once
Declare the stage budgets of the README (2/7/3/2/1, total 15 minutes) in `.ai/repo/ci/release.yaml`
(the canonical CI model of the release), schema-validated, read by every consumer. No number in
workflow YAML or prose that the model does not hold.
Acceptance: schema + validation case; `majordomus` reads it; docs/DISTRIBUTION.md renders it from
the model.

## Issue 2: Each release records its timeline
From the GitHub API (run, jobs, steps, Pages builds) and the site (`releases/latest.json`, polled),
record per release: tag push, plan done, each archive uploaded, release published, metadata pushed
to gh-pages, Pages build done, latest.json served, each smoke target green. Store it as a tracked
release-timeline record beside `.ai/repo/releases/<tag>.yaml`, written by the release's own record
step or by a typed capability that reads the run. Backfill v0.19.1 and v0.20.0 from the runs named
in the README.
Acceptance: a case with a scripted GitHub API; the backfilled records match the README's table.

## Issue 3: Over budget is a finding, with the stage
`release-check` (or the capability of prompt 07) reports a release whose total or any stage is over
budget, naming the stage, the measured seconds and the budget. Not a hard gate on publishing (a
release that is out must stay out); a gate on the *next* release's planning: it lists the stages
that ran over last time.
Acceptance: positive and negative case; the finding names stage and seconds.

## Issue 4: The site shows it
The release page on majordomus.dev shows the latest release's timeline against the budget, derived
from the record (no hand-written table).
Acceptance: site-check proves the numbers equal the record's.

## Completion report
Revision; the budget as declared; the backfilled timelines; findings for v0.19.1 and v0.20.0.
