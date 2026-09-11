---
schema: adr/v1
id: adr-0045
kind: adr
title: A repository is landed only when every stage of delivery says so
status: proposed
date: 2026-09-11
tags:
  - architecture
  - governance
  - process
related:
  - adr:adr-0004
  - adr:adr-0030
  - rule:majordomus.completion-gates
  - rule:majordomus.obligation-closure
  - rule:project.never-reported-is-not-green
  - file:apps/majordomus-cli/src/capability/builtin/landing.rs
  - file:test/cases/137_landing_closure.sh
provenance:
  origin: authored
---

# 45. A repository is landed only when every stage of delivery says so

## Context

Two questions in this repository already refuse a worker's claim of completion, and both are
scoped to one worker's change.

`obligations.closure` (ADR 0030) answers *what does this task still owe*: the tokens of
`share/obligations.yaml` the task declared, each judged against the ledger and against the
hash of the files it names. `gates.completion` answers *have the validation gates this
change selects reported over this tree*, out of `.ai/repo/ci/gates.yaml` and the same ledger.
Between them, `majordomus finish` cannot be talked into accepting a task whose tests never
ran.

Neither answers about the repository. That gap is not theoretical. On 2026-09-11 this
checkout held 104 local branches, 82 of which the trunk did not reach; 30 worktrees carried
uncommitted work; 62 branches held commits no remote had; eight branches were merged and
still existed. Every one of those tasks had been finished. Several of them had been finished
*correctly* — the task closure was right, and the repository was still nowhere near
delivered.

Each of those facts already had an engine that knew it. `worktree.topology` knows every
branch's upstream distance and whether the trunk reaches it. `artifacts.list` knows which
derived file no longer matches its source. `deploy.check` knows which declared deployment
would refuse. `distribution.status` knows whether the advertised installation would work.
`plan.status` and `plan.validate` know what the project model says is outstanding.
`health.report` knows the layer's own diagnostics and the benchmark coverage. What did not
exist was a verdict over them, and a fact with no verdict is a fact nobody is held to.

## Decision

**Landing is a closure over delivery, composed from the engines that already decide each
stage, and never a model of its own.**

`landing.closure` reports twelve stages — implementation, tests, documentation, generated
projections, commit, push, integration, CI, deployment, publication, project model, workspace
cleanup — in the order delivery happens in. Each stage is one execution of the capability
that owns the answer, made through the same executor MCP and the HTTP routes use, and
translated once into the `GateStatus` vocabulary the `gates` module already declares.

Three properties follow, and they are the decision:

1. **No stage is measured here.** The module reads no file, runs no git command and holds no
   threshold. Adding a stage means naming a capability that answers it. A stage this
   repository cannot answer is reported as such rather than given a rule of its own — which
   is the same refusal ADR 0004 makes about a semantic definition repeated across
   projections.

2. **Absence is never a pass.** A stage whose owner could not be executed is `unknown` and
   carries the reason; a stage whose owner has nothing recorded is `queued`. `landed` is
   false for both. The alternative — reading silence as delivery — is the exact failure the
   task-level doctrine already refuses, applied one level up
   (`project.never-reported-is-not-green`).

3. **One declaration, every surface.** The capability is declared once under
   `apps/majordomus-cli/src/capability/builtin/`, and the command line (`majordomus
   landing`, exit 10 when not landed), the HTTP route, the OpenAPI document, the MCP tool and
   resource, and the registry pages are projections of it.

## Consequences

A repository-wide verdict is now a thing a person, a script or an agent can ask for, and it
names what to do about each refusal rather than reporting "not ready".

The verdict is honest about its reach and says so in the report. Three facts are not local:
whether the trunk's own CI run is green, whether the published site serves the trunk's
commit, and whether the advertised installation works from a machine that has never seen the
project. The first is answered from recorded gate runs when this checkout has them and
reported as `queued` when it does not; the second and third are the release workflow's smoke
phase and `scripts/pages verify`. None of them is guessed.

The report is only as good as its sources, which is the point: a stage that reads wrong is a
defect in the engine that owns it, fixed once, and every surface — including this one —
improves together. The failure mode it deliberately accepts is cost: `worktree.topology`
costs one subprocess per registered worktree, so the closure is not a hot path and is
declared with caching disabled, because a landing verdict cached for even seconds is a
verdict about a tree that has already moved.
