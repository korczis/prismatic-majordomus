---
schema: moment/v1
id: three-roadmaps-none-of-them-true
kind: moment
title: 'Three roadmaps, and none of them true'
short_title: 'Three roadmaps'
hook: 'read three roadmaps for one project and believed none of them'
summary: 'A plan kept in prose is a snapshot, and every copy of it drifts on its own schedule; stored status is an assertion that stays true after the world moves.'
status: stable
severity: medium
frequency: common
weight: 100
featured: true
audiences: [engineering-lead, platform-team, open-source-maintainer, enterprise]
areas: [work-tracking, documentation]
lifecycle: [planning, maintenance]
tags: [plan, roadmap, status, drift]
signals:
  - id: copies-disagree
    text: 'More than one document describes the plan, and they do not agree.'
  - id: status-is-typed
    text: 'Status is a field somebody typed rather than something derived from what happened.'
  - id: started-out-of-order
    text: 'Work started on something whose prerequisite was not finished.'
examples:
  - id: readme-github-doc
    audience: engineering-lead
    title: 'README, milestones, planning document'
    before: 'An issue is closed on GitHub, open in the document and absent from the README, and somebody has just started the work that depends on it.'
    after: 'One canonical model; the roadmap and the GitHub milestones are projections of it, written one way.'
  - id: derived-status
    audience: platform-team
    title: 'A status field nobody could keep true'
    before: '"Done" typed into a field stays true after the dependency it relied on was reverted.'
    after: 'No status is stored: READY, BLOCKED, ACTIVE, VERIFY and DONE are derived from recorded events and the dependency graph, every time.'
  - id: public-roadmap
    audience: open-source-maintainer
    title: 'A public roadmap that stopped being true'
    before: 'The published roadmap is a hand-maintained list that nobody updates once the first item slips.'
    after: 'The roadmap page is generated from the plan; there is no second list to update.'
commands: [plan, doctor]
capabilities: [graph.get, graph.list]
claims: [project-schema, project-status-derived, dag-validation, execution-waves, evidence-gates-done, roadmap-derived, github-projection]
doctrines: [majordomus.project-integrity, majordomus.dag-integrity, majordomus.roadmap-integrity, project.derived-files-regenerated]
use_cases: [plan-the-work-as-data, deliver-issues-in-waves, complete-an-issue-only-with-its-evidence]
related: [milestone-status-unreconstructable, code-without-an-issue, three-copies-of-one-explanation]
aliases: ['conflicting roadmaps', 'stored status drift', 'plan in prose']
---

## The moment

The README has a roadmap. GitHub has milestones. A planning document has a diagram. They
were the same plan in June. Today an issue is closed on GitHub, open in the document and
absent from the README, and a worker has just started the piece of work that depends on it.

## Why it happens

A plan written in prose is a snapshot, and every copy of it is a second snapshot that drifts
on its own schedule. Status is the worst of it: "done" typed into a field is an assertion,
not an observation, and it stays true in that field after the world has moved on. Work is
then picked from memory rather than from a dependency order, so something starts before the
thing it needs, and the diagrams are redrawn by hand until someone stops redrawing them.

## Why a better model does not fix it

A worker asked to pick the next task reads whichever copy it was pointed at. It cannot know
that two other copies exist, and it certainly cannot know which of the three is least stale.
Handing it all three produces a worker that has to guess, which is what the humans were
doing.

## What it costs

Work started out of order, which shows up much later as a rewrite. Planning meetings spent
reconciling documents rather than deciding anything. And the slow abandonment of all three
copies, after which the plan lives in a few people's heads.

## What Majordomus does

The plan is two kinds of canonical file, checked against an allowlist where a key nobody
reads is an error. A milestone is an executable specification of an outcome. An issue is a
bounded execution contract: the paths it may touch, the issues it depends on, its acceptance
criteria and the evidence its completion requires. No status is stored anywhere. `READY`,
`BLOCKED`, `ACTIVE`, `VERIFY` and `DONE` are derived from what an issue records about itself
and from the state of its dependencies, every time `majordomus plan` runs; a written status
field is an unknown key.

`plan validate` refuses a cycle, a self-dependency and a dependency on an issue that does
not exist, each by name. `plan waves` computes the execution order from the graph and
reports issues in one wave that touch the same paths as serialised. `plan start` refuses an
issue that is not ready, naming what it waits on; `plan done` refuses while any declared
evidence is uncovered or a dependency is not done. The roadmap on this site and the
milestones on GitHub are projections of the same files.

## Before and after

```text
before   README roadmap | GitHub milestones | planning doc     three snapshots

after    $ majordomus plan next
         I0813  READY   session-knowledge-integration   (deps I0806, I0812 done)
         $ majordomus plan done I0042
         refused: evidence 'gap_reproduced' is uncovered
```

## How to verify it

Add a dependency edge and run `plan next`: the ready set changes without any status being
edited. Introduce a cycle and `plan validate` names it. Nothing anywhere carries a status
field, because the allowlist refuses one.

## What it does not do

It does not estimate, schedule or prioritise; it orders by dependency and reports what is
ready. The model is opt-in: a repository without one is skipped by `doctor`, not failed. The
projection runs one way — GitHub is written from the files, and nothing written on GitHub
is ever copied back into them.
