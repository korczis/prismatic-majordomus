---
schema: knowledge/v1
id: none-of-the-failures-of-2026-10-28-was-prevented
kind: knowledge
class: lesson
title: Eleven failures of 2026-10-28, and why none of them was prevented
description: The incident review of 2026-10-28 - eleven failures with their verified root causes and evidence, what already covers each, the issues filed to prevent each, and what is still owed.
status: verified
epistemics: observed
date: 2026-10-28
tags:
  - post-mortem
  - ci
  - runners
  - disk
  - mesh
  - release
provenance:
  origin: authored
  derived_from:
    - file:scripts/ci/release-check
    - file:.github/workflows/validate.yml
    - file:.github/workflows/release.yml
    - file:apps/majordomus-cli/assets/mesh-cache-provider/disk-guard
    - file:apps/majordomus-cli/src/mesh/cache_provision.rs
    - file:apps/majordomus-cli/src/mesh/cooperation.rs
    - file:scripts/reap-orphans
    - file:scripts/ci/reference-check
    - file:lib/rust_bin.sh
relations:
  - type: documents
    target: file:.ai/repo/project/issues/I4341.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4342.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4343.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4344.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4345.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4346.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4347.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4348.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4349.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4351.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4353.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4354.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4355.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4356.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4357.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4358.yaml
  - type: documents
    target: file:.ai/repo/project/issues/I4359.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I2674.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3241.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3903.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3072.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3825.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I2282.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3959.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I4006.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3974.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3810.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I3990.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I2054.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I2712.yaml
  - type: relates_to
    target: file:.ai/repo/project/issues/I1954.yaml
  - type: relates_to
    target: file:.ai/repo/rules/project/reclaim-only-what-you-own.v1.md
  - type: relates_to
    target: file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
---

# Eleven failures of 2026-10-28, and why none of them was prevented

## What was observed

2026-10-28. Eleven failures in one day, each read by an analyst and checked by a skeptic
against origin/master 81ef931a3e1; where the skeptic corrected or refuted an analysis, the
corrected root cause is the one below. Nothing was changed, built or cancelled to reach
them.

| # | Failure and its evidence | Verified root cause | Already covering | Filed |
|---|---|---|---|---|
| 1 | Lundra's runners lundra-1..3 went offline when its disk filled at about 12:00Z, during run 40680986818 (also 10-18 and 10-22) | No enforced free-space floor on lundra: I2674's disk-guard and ballast were never applied there, and no check fails while they are missing. Three hand-provisioned runner homes each keep an unbounded cargo build directory. The listener then crash-looped inside a unit systemd still showed active | I2674 is on master but not applied, and its disk-guard says HOMES=(/home/korczis). I4352 (fix/lundra-provisions-its-runners, not landed) renders the runner homes and drop-ins. #2596 routes more CI there. I3959 and #2567 never reach CI | I4343, I4344, I4345, I4346, I4359 |
| 2 | Hosted validate checkouts timed out at 600 s, six times across runs 40708407384 and 40733537887 (both attempts) | Every `fetch-depth: 0` checkout fetches 2.85 GiB: 549 heads, every tag and every blob. Multi-MB derived files are re-committed on almost every commit, and registry.json alone is 1.76 GB packed. Concurrent full clones meet a fixed 600 s bound. Nothing budgets checkout time or repository size, and a re-run changes nothing | #2633 (I4296, open) fetches blob:none for structure only and raises its bound to 20 minutes, a mitigation. I3241 was p3 and not started (raised to p0 in this change) | I3241 (amended), I4349, I4351 |
| 3 | Trains 10a (#2630) and 10b (#2631) waited 73 and 108 minutes | Each train's new run was pending in its own validate-<pr> group behind an older run of the same pull request, a superseded run or a same-head twin; cancel-in-progress: false and the integration/* exemption from first-red self-cancel kept those runs going for hours. Pool contention from held and resumed runs is secondary | #2645 (I3903) is open and defective (see 4). I4338 was never pushed, and it is a mitigation | I4347, I4353 |
| 4 | A superseded or duplicate run held a pull request's group: the fifth occurrence (10-10, 10-22, 10-26 and twice on 10-28) | `labeled` starts validate with no label filter, so a train opened with `--label train` gets two full runs of one head (6 of 8 trains). No automated actor that cannot be starved retires redundant runs | #2645 measures its bound from run_started_at, which for a run waiting in a group is its creation time, so it would have cancelled the legitimate run at about 00:48Z; it cannot see same-head twins and it runs on the starved pool | I4347, I4348 |
| 5 | The 641b Mac's free space fell to 0-3 GiB at 17:32Z and 21:34Z | Every reclaim of the shared build directory waits for cargo's profile flock (unfair, never free under concurrent builds), an empty profile-user list and a 60-minute idle age. Idle incremental sessions, about 55% of its size, could be removed lock-free under rustc's s-*.lock and are not. Steady headroom (27-30 GB) is below the 35 GiB floor | #2397 (I4006): its floor sits above the machine's steady free space, and raw cargo bypasses it. #2567 is still tied to the lock. I3959 evicted 0 bytes twice | I4354, I4355 |
| 6 | Id collisions: I4251 (#2601 and #2605), I4296 (#2633 and the deployed-stage worker) | Ids come from one global sequence and nothing reserves a number atomically: the board and `--advisory` claims with `\|\| true` are advisory, one admission mutex per runtime is eventually consistent, and the collision gate runs behind the build and is lost when the run cancels itself. A claim made through a stale runtime's closed session answered Written and expired at once | I3974 (planned) is a survey allocator, and ADR 0168 collided even with a survey. #2633 and I4337 are mitigations | I4356, I4357 |
| 7 | Master validate 40676150754 went red on case 103 after v1.0.0-alpha.9 was tagged before its record | release-check's two excuses do not cover the same time: "in flight" (I2712) ends when the run concludes, "proposed record" (I2054) reads only the refs fetched at checkout. A job that fetched after the tag and before the record branch's push, and judged after the run ended, matches neither; structure has the same gap. Every job turns red about 2026-10-29 12:47Z if #2626 has not landed | #2626 is open and blocked (action_required) | I4341, I4342 |
| 8 | Case 59 failed in pre-push three times on 10-28, ten times since 10-18 | reference-check's hand-typed is_exempt() leaves out docs/PLAN_STATUS.md, a pure projection of exempt records, so any planned, unwritten path in a record fails the check. Bare-directory scopes amplify it: 701 entries, 6513 conflict rows | Per-record symptom fixes only (cb2aa8187e6, cbcb3465f94, 5014fa07b4a), none on master. I4350 (committed in fbd9243c449 on chore/evolve-20261028, not pushed) refuses a directory scope over a planned file | not filed: no id left |
| 9 | The exclusive release/seat claim c-56b837289bb0 stayed live for more than 14 hours on the launchd hub | A claim lives by the heartbeat of the runtime stream that served it, not by its holder, and the hub never dies (KeepAlive, --idle 0). claim() silently opens an unknown session name (a hand-shortened id, run from another repository), so the session has no client_pid and the I3810 reaper never sees it. I3990 applies only to board sessions | I3810, I3990, I3174, #2410 and #2380 do not cover it | I4356, I4358 |
| 10 | The site contradicted itself (232 / 0 / stale supported), and `plan delivery` (0 deployed) disagreed with `delivery report` (29/29) | (A) The tracked evidence ledger has had no producer since 2026-09-17 (I1954 was never built), yet it is published as available and complete; rule published-evidence-is-what-ci-recorded covers only /ui/, and the homepage ignored ci.state. (B) plan delivery read site/static/releases/latest.json while gh-pages serves releases/latest.json; the reader failed open and its fixture was written to the reader's assumption | #2639 (I4315) is red (17 failed). I4317 and I4318 are pushed with no pull request | not filed: (B) belongs to #2639's review; (A)'s residue has no id left |
| 11 | reap-orphans deleted a live worktree's target, and ADR 0224 then refused every hook with exit 12 | The only copy of the executable the hooks require sits in a directory the sweep treats as regenerable cache, which stopped being true with ADR 0224. reap-orphans decides liveness from cwd and argv only, which a session working through absolute paths never shows, and each deleting sweep defines liveness its own way. The 21:43 deletion is evidenced; attributing the one before 17:55Z to the reclaim is an inference | The delivery.json half: I3836 (a57482183bf) and I3963 landed, with a residual fail-open. The target half: none (I3832 is not on master; I4324 widens the risk) | I4346 |

Six fixes existed and had not landed (#2645, #2397, #2567, #2639, #2626, #2633), and I4317
and I4318 had no pull request at all. That is the lesson: on 2026-10-28 every failure had
either no fix or a fix that was not on master, and none had an enforcement point that would
have failed before it happened.

## What prevents each

The issues below are filed in the review's order of priority. Each names the gate or case
that decides it; "L24/7" marks the ones that block lundra serving CI around the clock.

- I4341 (p0): release-check's excuse for a tag without a record is monotonic over its
  grace, whatever the job fetched. Case 3029 over every ordering.
- I4342 (p0): a release record's own pull_request run starts without a person, through a
  GitHub App or fine-grained token the owner supplies, and the release seat fails at two
  hours. A dispatched validate is not a remedy: I2282 showed branch protection does not
  count it (#866, #1273). Evidence is a real record reading CLEAN; case 4342 holds the bound.
- I4343 (p0, L24/7): a timer on the build host runs the doctor's observation every five
  minutes and publishes it on the mesh; it fails while the disk-guard timer, ballast, state
  file or drop-ins are missing. Lundra's evidence is recorded on I4343; I2674 keeps its S3
  criteria and is no longer a dependency, since its disk-guard code is on master.
- I4344 (p0, L24/7): each runner home's build directory gets a filesystem (or project
  quota) of its cap, so a job fails with ENOSPC in its home instead of filling the root;
  around it a self-hosted job bounds before and after cargo and coverage leaves no
  llvm-cov-target. Case 4344 and the job summary fact `build-dir peak`.
- I4345 (p0, L24/7): a root watchdog restarts a runner whose listener crash-loops inside an
  active unit, after reclaiming space. Case 4345.
- I4346 (p0, L24/7): one fail-closed liveness predicate, the typed capability
  worktree.liveness, for reap-orphans, worktree cleanup and disk-guard. Cases 611 and 276
  blocking; reclaim-only-what-you-own v2 blocking.
- I4347 (p0): a labeled event starts a gate job only for ci:full. Case 26.
- I4348 (p0): a blocking rule that a superseded or duplicate run never holds a validate
  group. Case 4348 in structure.
- I3241 (p0, amended here from p3): every checkout is blobless by default and fetches
  history only for a gate that declares history_reads; no checkout over 120 s.
  step-bound-check and case 3749.
- I4349 (p1): checkout seconds and repository size have a budget gate, the typed capability
  integration.checkout_budget. Case 4349.
- I4351 (p1): a tracked derived file over 1 MB is declared with a delta budget or leaves
  git, decided by an ADR first. derive-check in pre-commit and structure.
- I4353 (p1): a train's top goes first on the runner pool, and resume never re-runs a held
  member. Case 3027 and a classify.rs parity test.
- I4354 (p1): under disk pressure, idle incremental sessions are reclaimed without cargo's
  lock and a cargo shim admits a build before cargo locks; it states the one order of the
  admission floors (I4355's emergency, clause 7's 35 GiB held by I3825, the builder count,
  I4006's slot), which I3825 now names. Cases 3959 and 276.
- I4355 (p1): a machine publishes machine/<id>/disk on the mesh below 35 GiB and names the
  build lock's holders below 10 GiB. Case 4355 and the doctor.
- I4356 (p1): an exclusive claim needs a known session bound to its holder's process, and
  is never answered Written unless Held. Rust tests, case 3990, the claim rule at v2.
- I4357 (p1): issue, ADR and case numbers are reserved by a ref created on the forge (which
  refuses an existing one atomically and runs no local hook; pre-push skips refs/ids/*) and
  checked at landing, in a job that does not wait behind build. Cases 560, 566 and 4357.
- I4358 (p1): a seat is taken by `start --seat` under its provider session, through I3072's
  derived join, kept between that session's tasks and lost within a grace once it stops;
  the doctor names an unbound seat. Case 4358 and the cooperation rule at v3 (cases 3150,
  3824).
- I4359 (p1, L24/7): a blocking rule that no CI runs on a host whose runner layer is
  undeclared (the typed capability mesh.runner_routing in structure) or whose latest
  observation is missing, failing or stale (a runner job-started hook on the host), and a
  runbook for self-hosted runners in docs/CI.md. Case 4359.

The order for lundra serving CI 24/7 is I4352 (in flight), then I4343, I4344, I4346,
I4345 and I4359. Until I4343's observation is green on lundra and I4359's hook refuses a
stale one, routing CI there (#2596, MJ_RUNNER) is unsafe.

## What is owed and not filed here

- #2645 (I3903) must not land as reviewed: it should measure its bound from the first
  job's start, cancel a queued run of a head that already has a run in flight or judged,
  never cancel an integration/* run through queued_past_bound, run on a pool that cannot
  starve it with cancel-in-progress: false, and report a stale supervisor. These are
  review findings for its owner, not a new issue.
- #2639 (I4315) owes, before it lands, one declared path for releases/latest.json shared
  by site-deploy and plan delivery, a fixture produced by site-deploy, and a check that
  `plan delivery` and `delivery report` agree for one served commit.
- With no id left in the block, these items of the review wait for a new block: reference-
  check exempting exactly the declared record projections (incident 8); one producer of
  published evidence and rule published-evidence-is-what-ci-recorded v2 over every reader
  (incident 10A); a commit-keyed executable cache for a reclaimed worktree, amending ADR
  0224 (incident 11); case 103 judging only its fixture; plan validate refusing a
  directory scope with a ratchet (partly I4350); one scope_conflict finding per directory
  entry; delivery.json observation failing closed; remote heads holding only open work.
- The owner's acts: land #2626 before 2026-10-29 12:47Z; release c-56b837289bb0 on the hub
  (:8791 /api/v1/mesh/claims/release) or restart the hub on the alpha.9 binary.
- Not yet investigated: who cancelled release run 40681048692, whose workflow has
  cancel-in-progress: false; and what fills the baseline disk on lundra (korczis home
  159G, docker 53G, minio 29G, swap 17G) and on 641b (.colima 46G).

## What would have caught it earlier

- A number taken on the board is not reserved. The block I4340-I4359 claimed on the mesh
  for these issues was already used, on 2026-10-29, by I4340 (uncommitted, in the worktree
  of the evolve skill's branch, beside I4337), I4350 (committed in fbd9243c449 on
  chore/evolve-20261028, not pushed) and I4352 (uncommitted, in
  fix/lundra-provisions-its-runners); those three ids were left
  to their holders. Incident 6 recurred while its review was being filed.
- A fix that exists only on an open pull request prevents nothing. Every failure of the
  day had a cure somewhere, and the cure for incident 1 had been on master for days,
  unapplied, with no check that failed while it was missing.
- A remedy that is a mitigation should say so in its own record, so a reader does not take
  a longer timeout or a hold label for the cause removed.
