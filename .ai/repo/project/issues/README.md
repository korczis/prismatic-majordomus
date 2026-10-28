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

So a scope names the files the work changes: files that exist, and the exact path of each
file it will create. Never a directory other issues plan under (`test/cases`, `docs`,
`.ai/repo/rules`): the scope-conflict warnings of `plan validate` then name the files those
other issues plan and nobody has written yet, `docs/PLAN_STATUS.md` publishes the warnings,
and `scripts/ci/reference-check` refuses the missing paths, so case 59
(`test/cases/59_reference_integrity.sh`) fails in pre-push on a branch that touched none of
them. That happened three times on 2026-10-28, and each was repaired by narrowing the scope
to file paths. `plan validate` already computes those conflicts; issue I4350 makes it
refuse a directory scope that holds a file another issue plans, and until it lands nothing
but this paragraph decides it.

An issue id is taken on the board, not in the refs: two sessions took I4251 within two hours
on 2026-10-27, and on 2026-10-28 one session committed I4296 at 18:37Z while another had
held an exclusive mesh claim on that issue's file since 18:16Z; the
pushed one kept the id and the claim holder renumbered its work to I4315. Before writing the file, ask every ref for the id, read
the board and the mesh's claims (`majordomus_peers`, `majordomus_mesh_peers`), and announce
it with `majordomus_announce`, naming the id beside the paths, before the first commit that
carries it (rule `project.work-is-claimed-before-it-is-built`). Issue I3974 owes the
allocator and the gate that would decide this; until it lands, these steps are all there is:

```bash
git fetch --quiet origin
git log --all --oneline -i --grep 'I<id>'                        # any commit naming the id
git log --all --oneline -- .ai/repo/project/issues/I<id>.yaml   # any ref holding its file
```

A collision found later is resolved by renumbering the issue that was announced second.

The order of execution is not a number here. It is the dependency graph, validated as a
DAG, and `majordomus plan next` is the only correct answer to what may be worked on now.
