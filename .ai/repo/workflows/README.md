---
schema: context/v1
id: ai.repo.workflows
kind: context
title: Workflows
description: The multi-step processes a worker follows here, and when each applies.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/start.sh, lib/check.sh, lib/finish.sh, lib/handover.sh, lib/plan.sh]
---

# Workflows

Multi-step processes a worker follows in this repository. Each is a sequence of commands
and the facts that make each step correct, not a rule: the rules live under `../rules/`
and the workflows show when to apply them.

| workflow | when |
|---|---|
| [`task-lifecycle.md`](task-lifecycle.md) | every session: start, orient, checkpoint, check, hand over or finish |
| [`plan.md`](plan.md) | the repository has a plan under `../project/` and work is taken from it |
| [`continuity.md`](continuity.md) | resuming after a session ended, or leaving work for one that has not begun |
| [`use-cases.md`](use-cases.md) | a change reaches a capability a use case runs, or adds one no use case covers yet |
| [`foreign-repository-adoption.md`](foreign-repository-adoption.md) | admitting a repository not built around Majordomus, adopting it and recording the trial |
