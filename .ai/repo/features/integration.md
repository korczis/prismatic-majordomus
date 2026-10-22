---
schema: feature/v1
id: integration
kind: feature
title: Pull requests integrated one at a time against the current master
short_title: Integration
headline: The next provably safe change is merged, its landing is verified, and the decision is made again from the master that exists now, never from a list drawn up before.
summary: Every open pull request is classified against the current master with the evidence for its disposition; the planner names the next merge; drain merges it, verifies from git that it landed where it was decided, observes again and repeats; repair brings master into a pull request whose only conflict is derived output.
status: stable
weight: 35
domain: coordination
featured: true
areas: [coordination]
modules: [integration]
commands: [prs]
rules: [project.integration-follows-the-current-master]
docs: [docs/INTEGRATION.md]
adrs: [adr-0101]
cockpit: [integration]
related: [coordination, ci, worktrees]
tags: [pull-requests, merge, integration]
---

## What it does

`prs status` lists every open pull request with exactly one disposition — ready, waiting for
checks, needing master, conflicting, superseded, redundant and the rest — each with the
reasons and evidence it was decided on and the revisions it was decided against. `prs plan`
names what would happen next. `prs drain` merges the next ready pull request, then proves from
git that the merge commit sits on master's first-parent line directly after the master it was
decided against, with the head that was decided on, before it records the merge as landed;
anything else halts the drain by name. `prs repair` merges master into one pull request whose
only conflict is over derived files, and `prs cleanup` closes pull requests whose work is
already on master.

The relation to master is decided by git with the repository's own merge drivers, never by
the forge's `mergeable` flag, and every surface — command line, HTTP, MCP and the Cockpit's
integration page — answers from the same recorded observation.

## What it does not do

It does not merge a batch drawn up once and executed later: every merge voids the plan and
the next decision is made from the new master. It does not resolve authored conflicts, approve
reviews or arm the forge's auto-merge; a pull request that needs a person says which person
and why.
