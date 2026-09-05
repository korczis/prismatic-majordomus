---
id: several-agents-one-repository
kind: application
title: 'Several workers in one repository at once'
summary: 'More than one agent or person is changing the same repository in the same period, in worktrees or in sequence.'
weight: 2
status: active
fits_when:
  - 'Work is divided by paths and it matters that the division is checked rather than agreed'
  - 'Sessions hand work to each other and the handover has to survive the branch moving'
  - 'You need to know which checkout a task record belongs to'
does_not_fit_when:
  - 'One person works one repository at a time and nothing is ever handed over'
  - 'Coordination is the point and you want a scheduler; this tool reports overlap, it does not resolve it'
use_cases: [run-several-workers-at-once, hand-work-between-sessions, serve-the-layer-to-ai-clients, keep-decisions-out-of-the-transcript, open-and-close-a-session, resume-from-a-prompt-asset, deliver-issues-in-waves, read-back-what-happened, resume-in-the-right-worktree, carry-a-blocker-across-a-handover]
doctrines: [majordomus.scope-integrity, majordomus.state-consistency, majordomus.handover-integrity]
responsibilities: [scope, state, handover]
---

# Context

Each worker is individually reasonable. The cost appears between them: two tasks touching one file from different assumptions, a task record that travels with a branch into a checkout that never claimed it, a handover nobody can tell is stale.
