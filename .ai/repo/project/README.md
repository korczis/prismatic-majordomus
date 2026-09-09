---
schema: context/v1
id: ai.repo.project
kind: context
title: Project model
description: Milestones as outcomes, issues as execution contracts, and a dependency graph that decides what is next.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/project.sh, lib/project.awk, share/allow/project.txt]
---

# Project model

The canonical plan of this repository: `project.yaml` for the project itself,
`milestones/` for outcomes, `issues/` for the execution contracts that reach them, and
the dependency edges between issues for the order they may be executed in. Nothing here
is generated. Everything downstream is: the ready set, the Mermaid diagrams, the GitHub
issues and milestones and the website's roadmap all come from `majordomus plan`, which
reads these files and nothing else.

The roadmap and the diagrams are regenerated with the tree. GitHub is not: it receives the
projection when somebody runs `scripts/github-sync --apply`, deliberately. The gate that
holds the two together is `scripts/ci/github-check`, which reads the remote and fails when
it has stopped agreeing — because for five days it did, and nothing noticed
(`project.github-projection-gated@1`).

No status is stored. An issue records what happened to it — `started_at`, `verified_at`,
`completed_at`, its evidence — and the engine derives BLOCKED, READY, ACTIVE, VERIFY,
DONE or CANCELLED from those facts and from its dependencies. A status field would be a
second source of truth that drifts the moment a dependency changes.

```bash
majordomus plan next          # the issues whose dependencies are satisfied
majordomus plan show <id>     # one issue, its edges and its derived state
majordomus plan validate      # the graph: acyclic, every edge resolves
```
