+++
title = "integration.events"
description = "Every action the repository's executors recorded, from any of its worktrees, oldest first: the lease taken and given back, observations, selections, stale decisions, each act's attempt before it and its outcome after — merges with the master before and after, refusals, refreshes, verification failures and closures — each with a typed action, its actor, the pull request, the head, the decision's reasons and, on an act, the evidence it was decided on. The trail is one file under the common git directory, so every worktree reads the same one."
weight = 66
slug = "integration-events"
[extra]
id = "integration.events"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
