---
id: plan-the-work-as-data
kind: use-case
title: 'Keep the plan as milestones and issues the tool can validate'
summary: 'Declare milestones as outcomes and issues as execution contracts, and let the tool say what is ready and what is blocked.'
category: knowledge
status: active
target: guaranteed
actors: [maintainer, agent]
difficulty: intermediate
commands: [plan]
doctrines: [majordomus.project-integrity, majordomus.dag-integrity]
claims: [project-schema, dag-validation, project-status-derived, execution-waves]
responsibilities: [plan]
applications: [long-running-work, ci-gated-project]
scenario:
  setup: plan-model
  given:
    - 'a canonical project model with one milestone and two issues, the second depending on the first'
  steps:
    - id: validate
      run: ['plan', 'validate']
      note: 'every record parses, every reference resolves, the dependency graph is acyclic'
      expect:
        exit: 0
        stdout_contains: ['milestone', 'issue', '0 failure']
    - id: what-is-ready
      run: ['plan', 'ready']
      note: 'the issues nothing blocks'
      expect:
        exit: 0
        stdout_contains: ['I0001']
    - id: what-is-blocked
      run: ['plan', 'blocked']
      note: 'the issues waiting on another'
      expect:
        exit: 0
        stdout_contains: ['I0002']
  then:
    - 'status is derived from the records and git, never written into them'
    - 'an issue cannot be marked done without the evidence its milestone requires'
---

# Situation

The plan lives in a tracker nobody reads from the repository, or in a document whose status column is a matter of opinion. What is ready to start and what is blocked is an argument, not a query.

# Outcome

Milestones and issues are files under `.ai/repo/project/`; `plan validate` refuses a dangling dependency or a cycle, and `plan ready` and `plan blocked` are computed from the same records every time.
