+++
title = "A task on a repository whose mesh is enabled works on the mesh"
description = "A task on a repository whose mesh is enabled works on the mesh"
weight = 58
[extra]
kind = "rule"
slug = "majordomus-task-on-the-mesh-1"
identity = "majordomus.task-on-the-mesh@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/task-on-the-mesh.v1.md"
+++
{% raw %}

## Rationale

A repository enables its mesh so that every worker on every machine sees what every other
one is building. A protocol that holds only while each worker remembers to claim what it
works on is advice, and it fails exactly when it matters: in the Majordomus repository, on
2026-10-15, a new session reached the mesh only as a nameless connection, a task's scope
reached it only when the worker remembered, and a milestone written five days earlier to
change that had never reached a pull request. `start` claims the task's scope on the mesh
when the checkout's shared server answers; this rule is what makes that claim more than a
convenience.

## Required behaviour

When a tracked declaration under `.ai/repo/mesh/` says `enabled: true`, an active task's
record carries `mesh_claim:` — the key of its scope's advisory claim as the mesh session
`task-<id>`. `start` writes it when the checkout's shared server answers. `finish` claims the
scope again when the record has none, before the contract is judged, so a server that came up
after the task started is all a task needs.

## Failure behaviour

`check` reports the missing claim under `mesh`. `finish --outcome completed` refuses it, and
the refusal is recorded like every other, naming this rule. `partial`, `blocked`, `no_match`
and `failed` are not refused: a worker whose server never came up still has a way out. A
repository that declares no enabled mesh is not judged at all.
{% endraw %}
