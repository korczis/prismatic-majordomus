---
schema: domain/v1
id: coordination
kind: domain
title: Coordination
headline: 'Every worker declares the paths it will touch, sees who else is working and where, and hears about a collision before it is committed.'
problem: 'Agents collide. Parallel workers edit the same paths, or build the same thing twice, without knowing about each other.'
status: stable
weight: 20
tags: [coordination, scope, worktrees]
---

# Coordination

Several workers on one repository at once: the scope a task claims, the board every
attached session announces itself on, the worktree each branch lives in, the plan whose
status is derived rather than stored, the machines that share one repository, and the
models and advisors a worker is routed to. A claim is information and a collision is
reported; what refuses a change is the task's own scope, at check and at finish.

Not in this domain: what the work must satisfy, which is governance; and when it counts as
finished, which is completion.
