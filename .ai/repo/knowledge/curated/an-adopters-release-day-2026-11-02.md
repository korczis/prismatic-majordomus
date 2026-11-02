---
schema: knowledge/v1
id: an-adopters-release-day-2026-11-02
kind: knowledge
class: lesson
title: An adopter's release day, 2026-11-02, and what Majordomus already holds for it
description: Six failures an adopter (OSCILLA, korczis/oscilla) met on 1.0.0-alpha.9 while landing a feature train and a release on a shared Mac, each mapped to the Majordomus record that already covers it; plus four working rules that held across seven sessions.
status: verified
epistemics: observed
date: 2026-11-02
tags:
  - adopter
  - release
  - ci
  - runners
  - disk
  - mesh
  - plan-evidence
  - trains
provenance:
  origin: authored
  derived_from:
    - file:.ai/repo/project/issues/I4410.yaml
    - file:.ai/repo/project/issues/I4320.yaml
    - file:.ai/repo/project/issues/I4326.yaml
    - file:.ai/repo/project/issues/I4466.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/project/issues/I4410.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4320.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4333.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4466.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4370.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4358.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4006.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4355.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4415.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4326.yaml
---

# An adopter's release day, 2026-11-02, and what Majordomus already holds for it

## What was observed

The adopter is OSCILLA (korczis/oscilla), working on Majordomus 1.0.0-alpha.9. Seven sessions, coordinated on the mesh, landed:
- feature train #367 (11cc9fbc);
- the v3.15.0 release (#382), whose gate needed four runs.

Every failure below was read from the adopter's history or a command run that day. Each one was mapped to an existing Majordomus record before anything was filed, and no new id was needed.

| # | Failure and its evidence | Root cause | Already covering |
|---|---|---|---|
| 1 | `plan evidence` recorded `commit: e0874ff8…`, a PR branch's merge commit. The adopter's PR-time check (tests/unit/plan-evidence-commits.test.mjs, V272 D4, active only with GITHUB_BASE_REF) failed in CI and passed locally. The fix was to re-record in a detached origin/main worktree (1d527725: e0874ff8 → ca1889c0). | The stamp is HEAD. On a PR branch HEAD is not a trunk commit, and under squash landing it never becomes one. | I4410 (`--commit`, refuses a commit the trunk does not reach) and I4320 (`plan validate` evidence_commit_off_trunk, `--restamp`) |
| 2 | #329 and #332 stayed open after their content landed through train #367: `git cherry origin/main <head>` showed 0 unapplied commits. | Nobody ran the relation for the adopter's PRs. | On master: ADR 0101 / WP13 `prs` reports `redundant (patch_ids_upstream)`. The squash case is I4333. |
| 3 | GitHub showed #330 as `CLEAN`, while `git merge-tree --write-tree --name-only origin/main <head>` found 27 conflicting files: 483 commits behind, including ci.yml, a guarded path. | The forge's mergeable flag is a cached instrument. A guarded path in the conflict set makes the recorded review verdict stale before anyone rebases. | Majordomus's own queue decides by merge-tree (docs/INTEGRATION.md). Verdict staleness is I4466. The pre-landing reconcile preview for adopters is noted against I4466. |
| 4 | The release freeze was coordinated in chat ("FREEZE, disarm #379"). An armed auto-merge PR could have landed under the release receipt mid-gate. | No machine-held freeze reached the adopter's armed PRs. | I4370 (release-freeze label), I4358 (release seat). Arming auto-merge is a declared non-goal. |
| 5 | The shared Mac fell to 4.7 GiB free with swap near full, and the release prepare's final doctor failed with ENOSPC. `du` placed most of the space in landed-branch worktrees (11 GB), session scratch (9.3 GB) and tool caches (14 GB). | Three sessions ran npm ci, builds and docker in parallel. Nothing showed disk, load and swap together, and no heavy job waited its turn. | I4006 (machine.slot and disk floor), I3825, I4355, I4323/I4343, I4415. Swap and memory pressure, and one board line for disk/load/swap, are noted. |
| 6 | For hours the adopter's repository had 20 jobs queued and 0 in progress, while githubstatus reported Actions as operational. | Account or hosted-pool starvation looked like slow CI. | I4326 (`forge_starved`), I4360. |

## Working rules that held

1. **Dry-run before planning a train.** Run `git merge-tree --write-tree --name-only <trunk> <head>` and `git cherry <trunk> <head>` for every candidate. They correct the forge's flags: one PR thought CLEAN had 27 conflicts, and two PRs thought open were already landed.
2. **Heavy jobs run one worktree at a time and wait for a peer's release gate.** On the adopter, the rule was a mesh message. It belongs in the machine slot (I4006).
3. **Map a finding to an existing record before filing.** All six findings above already had homes. Filing none was right, under the standing order to close more than you open.
4. **Split a many-surface order by owner, and share one component set.** The order was "update and connect every page". The adopter audited nine workspaces read-only, shared the audit, and split the pages by domain owner. One lane builds the shared pieces (workspace registry, page header with related links, empty state, chart and graph wrappers), and no lane imports a library on its own.

Adopter-side guidance, not a Majordomus rule: a train whose members include feat or fix changes must itself carry a feat or fix title wherever a fail-first judge keys on the conventional type (OSCILLA's fail-first.mjs, in its own scripts directory). Majordomus has no such judge.
