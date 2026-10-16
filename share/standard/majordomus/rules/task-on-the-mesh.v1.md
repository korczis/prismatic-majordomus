---
id: majordomus.task-on-the-mesh
version: 1
kind: rule
title: A task on a repository whose mesh is enabled works on the mesh
description: When the repository declares its mesh enabled, an active task's scope is claimed on the mesh — `start` claims it when the checkout's shared server answers and `finish` claims it again before judging — and a task completed without that claim is refused.
statement: On a repository whose tracked mesh declaration is enabled, a task is not completed until its scope has been claimed on the mesh as the session task-<id>; partial, blocked, no_match and failed are never refused for it.
status: active
class: blocking
depends_on: []
tags: [mesh, coordination, session, doctrine]

x-majordomus:
  validator: task_on_the_mesh
  category: mesh
  enforced_by: [check, finish]
  exit_code: 10
  claims: []
  tests: [test/cases/3068_a_task_off_the_mesh_is_refused.sh, test/cases/3067_a_task_is_claimed_on_the_mesh.sh]
---

# Rationale

A repository enables its mesh so that every worker on every machine sees what every other
one is building. A protocol that holds only while each worker remembers to claim what it
works on is advice, and it fails exactly when it matters: in the Majordomus repository, on
2026-10-15, a new session reached the mesh only as a nameless connection, a task's scope
reached it only when the worker remembered, and a milestone written five days earlier to
change that had never reached a pull request. `start` claims the task's scope on the mesh
when the checkout's shared server answers; this rule is what makes that claim more than a
convenience.

# Required behaviour

When a tracked declaration under `.ai/repo/mesh/` says `enabled: true`, an active task's
record carries `mesh_claim:` — the key of its scope's advisory claim as the mesh session
`task-<id>`. `start` writes it when the checkout's shared server answers. `finish` claims the
scope again when the record has none, before the contract is judged, so a server that came up
after the task started is all a task needs.

# Failure behaviour

`check` reports the missing claim under `mesh`. `finish --outcome completed` refuses it, and
the refusal is recorded like every other, naming this rule. `partial`, `blocked`, `no_match`
and `failed` are not refused: a worker whose server never came up still has a way out. A
repository that declares no enabled mesh is not judged at all.
