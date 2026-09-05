---
id: deliver-issues-in-waves
kind: use-case
title: 'Deliver issues in waves the graph computes, and take the next one'
summary: 'Validate the plan, read the execution waves the dependency graph yields, see which issues are ready and which are blocked on what, and take the one issue a worker should do now.'
category: workers
status: active
target: guaranteed
weight: 76
actors: [maintainer, agent]
difficulty: intermediate
commands: [plan, start]
doctrines: [majordomus.project-integrity, majordomus.dag-integrity, majordomus.scope-integrity]
claims: [dag-validation, execution-waves, project-status-derived, scoped-task, overlap-report]
responsibilities: [plan, scope]
applications: [several-agents-one-repository, long-running-work]
scenario:
  setup: plan-model
  given:
    - 'installed, with a canonical project model carrying one milestone and two issues'
  steps:
    - id: validate
      run: ['plan', 'validate']
      note: 'schemas, references, the DAG and status consistency; a cycle or a dependency on a missing issue is refused by name'
      expect:
        exit: 0
        stdout_contains: ['0 failure']
    - id: waves
      run: ['plan', 'waves']
      note: 'topological execution waves; issues in one wave that touch the same paths are reported as serialised'
      expect:
        exit: 0
        stdout_contains: ['^Wave 0', 'I0001 +READY', '^Wave 1', 'I0002 +BLOCKED']
    - id: ready
      run: ['plan', 'ready']
      note: 'issues whose dependencies are all satisfied'
      expect:
        exit: 0
        stdout_contains: ['I0001']
        stdout_not_contains: ['I0002']
    - id: blocked
      run: ['plan', 'blocked']
      note: 'issues waiting on a dependency, and on which one'
      expect:
        exit: 0
        stdout_contains: ['I0002']
    - id: next
      run: ['plan', 'next']
      note: 'the one issue a worker should take now'
      expect:
        exit: 0
        stdout_contains: ['^I0001 ', 'scope:', 'next: majordomus plan start I0001']
    - id: take-it
      run: ['start', 'I0001', '--scope', 'src/I0001']
      note: 'the task under the scope the issue names; overlap with other worktrees is computed on claimed paths'
      expect:
        exit: 0
        stdout_contains: ['t-']
  then:
    - 'status is derived from recorded facts and stored nowhere'
    - 'two issues in one wave with overlapping paths are named as serialised'
    - 'the task started carries the scope the issue claims'
---

# Situation

A plan with twenty issues and a handful of dependencies has an order in it that nobody wrote down. Workers pick what looks interesting, two of them take issues that touch the same files, and the dependency somebody forgot surfaces as a rebase.

# What you run

- `plan validate`: the model, references, DAG and status consistency
- `plan waves`, `plan ready`, `plan blocked`: the order the graph implies, and what is waiting on what
- `plan next`: the one issue to take
- `start <task> --scope <paths>`: the task under the issue’s scope, with overlap reported at start

# Outcome

The order is computed, not remembered. Parallel issues that would collide are named before anyone starts them, and the issue a worker takes is the one the graph says is next.
