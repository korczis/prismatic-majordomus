+++
title = "completion-is-proved — A development session is complete when the repository can prove it, and every surface reads the same proof"
description = "One completion policy, share/completion.yaml, is the definition of done. The completion report derives the lifecycle stage a task stands at from the answers to it, states whether the task is complete, and every surface - check and finish, the HTTP route, the MCP tool, the Cockpit, the generated section of every provider bootstrap, the site - reads that one report. The outcome completed is refused until the report says complete. The published site, the release metadata and every active deployment are asked what they serve and compared with what the trunk expects, and a stale or unreachable surface refuses. The release path runs the structural version gate."
weight = 20
template = "milestone.html"
[extra]
plan_id = "completion-is-proved"
source = ".ai/repo/project/milestones/completion-is-proved.yaml"
+++
