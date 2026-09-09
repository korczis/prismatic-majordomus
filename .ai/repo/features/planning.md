---
schema: feature/v1
id: planning
kind: feature
title: Milestones and issues as data; status derived, never stored
short_title: Planning
headline: An issue records what happened to it and the engine derives whether it is blocked, ready, active, verifying or done from the dependency graph, so the roadmap cannot disagree with the plan.
summary: Milestones are outcome specifications and issues are execution contracts, one YAML file each; the dependency graph decides execution waves and the next ready issue; a milestone whose dependencies are not accepted is blocked whatever its own issues say; and the GitHub projection is rendered offline from the same model.
status: stable
weight: 130
featured: false
areas: [work-tracking]
commands: [plan]
kinds: [milestone, issue]
rules: [majordomus.project-integrity, majordomus.dag-integrity, majordomus.roadmap-integrity]
docs: [docs/PLANNING.md, docs/ROADMAP.md]
claims: [project-schema, project-status-derived, dag-validation, execution-waves, evidence-gates-done, roadmap-derived, github-projection, task-dependencies]
use_cases: [plan-the-work-as-data, deliver-issues-in-waves, complete-an-issue-only-with-its-evidence]
related: [worktrees, finish-contract]
tags: [plan, milestones, issues]
---

## What it does

`majordomus plan validate` refuses a cycle, an edge that resolves to nothing and a record
that breaks its schema; `plan next` answers which issues may be executed now; `plan roadmap`
draws the milestone graph the website renders. A branch created for an issue is named after
it, so the worktree topology can read the issue back from the branch, and an issue is done
only with the evidence its contract requires.

## What it does not do

It does not talk to GitHub on its own: the projection of issues and milestones is rendered
offline and applied only when a person runs the sync with a token. Nothing here estimates
effort or schedules dates.
