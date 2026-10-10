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
| [`landing-train.md`](landing-train.md) | several pull requests are finished at once: the one executor, a link prepared on the one below, when it merges, what breaks the train, the runner budget |
| [`release-verification.md`](release-verification.md) | a release tag was pushed: the record, the attestations, the site, every machine and every hub, read back |
| [`plan.md`](plan.md) | the repository has a plan under `../project/` and work is taken from it |
| [`continuity.md`](continuity.md) | resuming after a session ended, or leaving work for one that has not begun |
