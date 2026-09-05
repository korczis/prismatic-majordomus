---
id: complete-an-issue-only-with-its-evidence
kind: use-case
title: 'Complete an issue only when its evidence exists'
summary: 'See an issue refuse completion while a required piece of evidence is missing, read the roadmap the milestone state derives, and keep the GitHub projection a projection.'
category: completion
status: active
target: guaranteed
weight: 54
actors: [maintainer, reviewer]
difficulty: intermediate
commands: [plan]
doctrines: [majordomus.project-integrity, majordomus.roadmap-integrity, majordomus.verification-integrity]
claims: [evidence-gates-done, project-status-derived, roadmap-derived, github-projection, project-schema]
responsibilities: [plan]
applications: [ci-gated-project, long-running-work]
scenario:
  setup: plan-model
  given:
    - 'installed, with a canonical project model carrying one milestone and two issues'
  steps:
    - id: status
      run: ['plan', 'status']
      note: 'milestone progress and the next executable issue, derived from recorded facts'
      expect:
        exit: 0
        stdout_contains: ['^M000 +PLANNED', 'next ready issue: I0001']
    - id: show
      run: ['plan', 'show', 'I0001']
      note: 'the full record of one issue, including the evidence it requires before it may complete'
      expect:
        exit: 0
        stdout_contains: ['^# I0001', 'READY.*derived', 'wave']
    - id: roadmap
      run: ['plan', 'roadmap']
      note: 'milestones in derived order, with now and next; no document is a second authority for this'
      expect:
        exit: 0
        stdout_contains: ['^now:  M000']
    - id: body
      run: ['plan', 'body', 'I0001']
      note: 'the provider-neutral projection body of one record, the same one the GitHub projection renders'
      expect:
        exit: 0
        stdout_contains: ['^# I0001']
  then:
    - 'completion is refused while a required evidence item is missing'
    - 'the roadmap is derived from milestone state, never written'
    - 'a hand edit inside a generated GitHub region is reported rather than kept'
---

# Situation

An issue is marked done because the person doing it was done. The test it required was never written, the document it promised never appeared, and the milestone advances on a claim. A roadmap maintained by hand says something else again.

# What you run

- `plan status`, `plan show <id>`: derived status, and the evidence an issue requires
- `plan roadmap`: milestone order derived from state
- `plan body <id>`: the neutral projection body the GitHub sync renders from

# Outcome

An issue completes only when its evidence exists in the repository, status is derived and stored nowhere, and the roadmap and the GitHub issues are projections of the one model rather than second authorities.
