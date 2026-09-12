---
schema: adr/v1
id: adr-0057
kind: adr
title: Completion is proved, not claimed — one completion policy, a derived stage, and live verification of what was deployed
status: proposed
date: 2026-09-12
tags:
  - governance
  - lifecycle
  - release
  - deployment
provenance:
  origin: authored
related:
  - rule:project.completion-is-proved
  - rule:project.deployment-is-verified-live
  - rule:project.tooling-is-derived
  - rule:project.the-version-is-measured
  - rule:project.never-reported-is-not-green
  - test:test/cases/280_completion_is_proved.sh
  - test:test/cases/281_deploy_verify_live.sh
  - test:test/cases/282_tooling_is_derived.sh
---

# 57. Completion is proved, not claimed — one completion policy, a derived stage, and live verification of what was deployed

## Context

Measured on 2026-09-12 against master `3407ca2d4`, four things were true at once.

**Done was stated in five places that could not disagree loudly.** The policy's
`verification.finish_requires` selected seven shell validators; `share/obligations.yaml`
named eleven tokens a task may owe; `.ai/repo/ci/gates.yaml` named forty-five gates;
`gates::done` carried nineteen questions as a Rust literal; and the provider bootstraps
(`AGENTS.md`, `CLAUDE.md`) restated the lifecycle in prose, from templates that duplicated
whole paragraphs by hand. Each was right about its own half, and the only reconciliation
was a reader's.

**The completion gate could not refuse.** `gates.completion` answered `finishable: true`
whenever no *recorded* gate run failed — and no run had ever been recorded in any
checkout's ledger, because nothing ran the gates through the verb that records them. A
gate that never reported was listed as `unverified` and did not block; choosing the
outcome `partial` skipped the verification, obligation and gate validators entirely; the
pre-push hook ran the `check` dispatch and not the `finish` one. `finish --outcome
completed` had never been refused by a gate, and structurally could not be.

**The version was measured and the release path never asked.** ADR 0051 made the
structural analysis (`release.analysis`, `scripts/ci/version-matches-surface`) the
authority for what a release's number must be, and `validate.yml` ran it when a change
landed. `release.yml` ran `scripts/release-version --check --tag` — the two writers agree —
and nothing else. Pushing a tag whose number matched both writers was the one way past the
gate. The commit-based report beside it, `release.version`, raised the *declared* version
by the bump the commits implied, so a tree already bumped for its window (0.6.0 over
0.5.0 released) was told its next version was 0.7.0.

**Deployment and verification were hand-held claims.** `share/obligations.yaml` declared
`deploy` and `verify` with `established_by: none`; a worker recorded what they saw. The
published site had a probe (`scripts/pages verify`), run in the Pages workflow under
`continue-on-error: true`, and a gate (`pages-live`) that no path class could ever select
and that ran nightly. Nothing asked a running executable which commit it was, though
`/api/v1/distribution/build` had stated it since the distribution model existed. A deploy
command that exited 0 with the old revision still live was indistinguishable from a
deployment — the repository had paid for exactly that twice (2026-09-09, 2026-09-10) and
a person had noticed by eye each time.

## Decision

**One completion policy.** `share/completion.yaml` is the definition of done: the lifecycle
stages in order, and every question a task must answer, each naming the *source* its
answer is taken from — an obligation token, the gate aggregate, one gate, the change set,
the structural release analysis, the task record's issue, the continuity store. The Rust
literal is gone; `gates::done` reads the policy and answers each question from its source
and from nowhere else. `gates.policy` serves the policy and the problems it has against the
repository (a token the vocabulary lacks, a gate the model lacks). The provider bootstraps
carry it as a generated fragment (`COMPLETION_CONTRACT`), rendered from the same file
by the shell tool and the executable and held to the same bytes by `generate --check`.

**A derived stage, and one bit.** The completion report folds the answers over the
policy's stages: the first stage with a refusing question is where the task is blocked,
else the first with an unanswered one is where it stands, else it is complete. `stage` is
never written by anybody. `verified` says every selected gate reported over this tree and
none refuses; `complete` says every question passes or is exempt. `complete` is the one
word for done, and no surface computes it a second time.

**`completed` is earned.** With `verification.completed_means_complete: true` — on in this
repository, shipped off with the reason in the skeleton — `finish --outcome completed` is
refused while `complete` is false, naming each question owed and the command that settles
it. `majordomus evidence --run-gates` runs every gate the change selects through the same
dispatcher CI uses (`scripts/ci/run-plan`) and records each exit as it finishes. The weaker
outcomes stay honest statements and now record the stage they stopped at, and a derived
handover carries a `Completion` section, so the next worker does not rediscover what was
left.

**Applicability is derived, and so are the obligations.** The deployment plan
(`deploy::targets`) says which surfaces a change reaches — the published site when the
change touches what the site is built from, the release when it touches what the public
surface is taken over, an active deployment object when it touches its build inputs —
from the CI model's path classes, the site's declared origin, the release records and the
deployment objects. A task owes `pages`, `deploy` and `verify` when the plan says the change
reaches those surfaces, and `push` and `target` whenever it changed anything, whether or
not it declared them; a token the change implies can be discharged, which declares it on
the record. A target that does not apply is in the plan with the reason.

**Live verification.** `deploy.verify` asks each applicable target for the identity it
states — `/build.json` for the site, `releases/latest.json` for the release metadata,
`/api/v1/distribution/build` for a running executable — and compares it with what the
trunk expects. A surface stating an older identity is **stale**; one that does not answer
is **unreachable**; a 200 with no identity is **unreadable**; none is a pass. The `deploy`
and `verify` obligations are established live by it, once the trunk reaches the task's
commit; the request carries no header and the evidence no body beyond the fields compared.
The one capability of the executable that reaches the network reaches only addresses the
repository itself declares.

**The contract gate is on the release path.** `.ai/repo/ci/release.yaml` carries a
`contract` phase and `release.yml` runs `scripts/ci/version-matches-surface` in the plan
job, before a single artifact is built. `release.version`'s `next` is raised from the last
release and never below what the tree declares.

## Alternatives rejected

**Make `finishable` false when a gate never reported.** It would have refused every finish
in a repository where recording was new, which is what ADR 0038 was avoiding, and it would
have folded two facts — refused and silent — into one word. The word `verified` carries the
second fact; the policy bit decides whether silence refuses.

**A state a worker sets.** `implementing`, `validated`, `deployed` as a field of the task
record. A state a worker sets is a claim, and the artifact a worker learns to advance. The
stage is a fold over evidence, and there is no field to set.

**A second implementation of the judgement in the shell tool, or in a surface.** The shell
validator reads `gates.completion`'s `complete` and never re-derives it; the Cockpit, the
site and the bootstrap fragment are projections of the same report and the same policy.

**Verifying by reachability.** An HTTP 200 from the site was the evidence before, and it
was true on both days the site served the wrong tree.

**Deploying the declared Fly.io object to make "every session deploys" literal.** Nothing
runs it and nothing is authenticated to run it; the object stays `declared`, the plan says
so by name, and the token is not pretended about.

## Consequences

- Every surface — `check` and `finish`, `GET /api/v1/gates/completion`, the MCP tool, the
  Cockpit, the generated section of every bootstrap, the site's lifecycle page — reads one
  report and one policy. A question added to `share/completion.yaml` is asked everywhere
  on the next generation, and a question nothing answers is a reported problem.
- This repository's own sessions pass through the lifecycle: the first was the one that
  built it (issue I1513, milestone `completion-is-proved`), and its finish record carries
  the stage and the evidence.
- A fresh repository is not refused every completion: the skeleton ships the bit off, with
  the reason, and `check` reports the same questions without refusing until it is turned
  on. The obligation and gate validators are in the skeleton's finish contract now, which
  they were not.
- Live verification costs a network request per applicable target at `check` time once
  the task is integrated. It is bounded (twenty seconds per target), reaches nothing the
  repository does not declare, and is the price of the difference between "deployed" and
  "the deploy command exited 0".
- The commit-based `release.version` report is kept as evidence beside the structural
  verdict and never above it; its `next` no longer overstates a bumped tree.
