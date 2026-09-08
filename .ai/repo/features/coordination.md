---
schema: feature/v1
id: coordination
kind: feature
title: Several agents on one repository, without stepping on each other
short_title: Coordination
headline: Every worker declares the paths it may touch, every session can see who else is attached and what they announced, and a collision is reported before it is committed.
summary: A task claims its scope before the first edit; check and finish refuse a file outside it and report another worktree's overlapping claim; every client of the shared server is a peer that can announce its intent and paths; and each branch has exactly one worktree, derived from git.
status: stable
weight: 30
featured: true
areas: [coordination]
audiences: [ai-native-team, platform-team]
modules: [peers, worktree]
commands: [start, check, finish]
rules: [majordomus.scope-integrity, majordomus.isolated-parallelism, majordomus.one-worker-one-scope, project.worktree-topology, project.shared-server-resilience]
docs: [docs/WORKTREES.md, docs/MCP.md, docs/ADOPTION.md]
adrs: [adr-0003, adr-0021]
claims: [scoped-task, scope-enforcement, overlap-report, mcp-peers, worktree-ownership]
use_cases: [run-several-workers-at-once, work-on-a-branch-in-its-canonical-worktree]
cockpit: [worktrees]
web: [mcp]
related: [worktrees, continuity]
tags: [agents, parallelism, scope]
---

## What it does

Parallel work needs isolation and visibility, and the tool gives both without a person
coordinating by hand. `majordomus start` records the paths a task may touch; `check` and
`finish` fail on a touched file outside them and report the overlapping claim of any other
worktree of the same repository on the same machine. Every branch that is worked on has one
worktree at a path derived from git's own identity, so two sessions cannot occupy one path
by accident and a handover can name where the work is.

The shared server makes the live half visible: every attached client — Claude Code, Codex,
Gemini CLI, a script — is a peer named by its own handshake, and `majordomus_announce` tells
the others one line of intent and the paths it expects to touch, before the first tool call.

## What it does not do

Overlap is reported, never blocked: the tool refuses to accept work as completed when a file
is outside the declared scope, and it does not stop an editor from writing the file while
the task runs. An announcement is informational and lives in the server's memory; the
durable, enforced form is the task record and its scope.
