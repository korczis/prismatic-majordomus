---
schema: context/v1
id: ai.repo.project.issues
kind: context
title: Issues
description: Execution contracts: one bounded change each, with its scope, its dependencies and the evidence it must leave.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/allow/issue.txt]
---

# Issues

One YAML file per issue, named for its `id`. An issue is an execution contract for one
bounded change: what it is for (`objective`, `why`), the state it was written against
(`current_state`, `desired_state`), the paths it may touch (`scope`, `non_scope`), the
profile it should be worked under, whether it is safe to run beside its siblings
(`parallel_safe`), and the issues it depends on. The keys are closed by
`share/allow/issue.txt`.

An issue carries no status field. It records events — when it started, when it was
verified, when it completed, and the evidence attached — and the engine derives the state
from those and from the graph. Two issues that touch one file are reported as unsafe to
run concurrently; that report is why `parallel_safe` exists and why scope is a pathspec
list rather than a paragraph.

The order of execution is not a number here. It is the dependency graph, validated as a
DAG, and `majordomus plan next` is the only correct answer to what may be worked on now.
