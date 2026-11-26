---
schema: knowledge/v1
id: a-background-agent-is-read-before-it-is-asked-again
kind: knowledge
class: lesson
title: "Read what a background agent did before asking it, or another agent, to do it again"
description: "When a delegated or background worker seems silent, stuck or finished, before re-sending its task, resuming it or spawning a replacement"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - delegation
  - multi-session
  - continuity
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/execution-state-is-authoritative.v1.md
    - file:.ai/repo/knowledge/curated/naive-fan-out-burns-tokens.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/execution-state-is-authoritative.v1.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/naive-fan-out-burns-tokens.md
---

# Read what a background agent did before asking it, or another agent, to do it again

A background worker that looks idle is not known to be idle. It may still be running, it may
have committed half the work, or it may have stopped at a usage limit with its branch pushed.
Re-sending the task without reading that state starts a second copy of the same work on the
same files, and the two copies then collide.

Observed on 2026-11-25 across one long multi-session run on a shared workstation:

- Several delegated agents hit account usage limits mid-task and had to be resumed by hand.
- Their work survived only where it had been committed and pushed. A resumed agent that was
  not first told what its predecessor had left redid work or met it as a conflict.
- Two sessions also executed the same plan issue because the second picker ran before the first
  session's pull request existed.

What to do, in order, before any re-send, resume or replacement:

1. Read the worker's own state: its output file or transcript tail, its branch and commits
   (`git log`, `git status` in its worktree), its task record and its board announcement.
2. Decide from that state: still running (wait for its notification), finished (consume its
   result), stopped (resume it with a brief naming what is already done).
3. Only then send. A resume brief names the last commit, what is pushed and what is not.

Polling a worker's progress in a loop is not reading its state. One deliberate read is.
