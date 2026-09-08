+++
title = "Several agents on one repository, without stepping on each other"
description = "A task claims its scope before the first edit; check and finish refuse a file outside it and report another worktree's overlapping claim; every client of the shared server is a peer that can announce its intent and paths; and each branch has exactly one worktree, derived from git."
weight = 30
[extra]
id = "coordination"
status = "stable"
source = ".ai/repo/features/coordination.md"
+++
{% raw %}

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
{% endraw %}
