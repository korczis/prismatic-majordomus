---
schema: moment/v1
id: issue-says-done-tests-disagree
kind: moment
title: 'The issue says done and the repository disagrees'
short_title: 'Done on paper'
hook: 'closed an issue whose acceptance criteria nothing had actually checked'
summary: 'Completion is recorded as a state change in a tracker rather than as evidence in the repository, so the two drift immediately.'
status: stable
severity: high
frequency: common
weight: 220
audiences: [engineering-lead, enterprise, ai-native-team, platform-team]
areas: [work-tracking, verification]
lifecycle: [review, operations]
tags: [evidence, completion, tracking, tests]
signals:
  - id: closed-not-proven
    text: 'Something was closed as done without anything checking its acceptance criteria.'
  - id: tracker-vs-repo
    text: 'The tracker and the repository disagree about what is finished.'
  - id: evidence-is-prose
    text: 'The evidence recorded for a completed item is a sentence somebody wrote.'
examples:
  - id: criteria-unchecked
    audience: engineering-lead
    title: 'Four criteria, none checked'
    before: 'An issue with four acceptance criteria is closed because the work "feels done"; two of them were never implemented.'
    after: '`plan done` refuses while any declared evidence is uncovered, and names which.'
  - id: evidence-is-a-command
    audience: platform-team
    title: 'Evidence that is a command, not a claim'
    before: 'The evidence field says "tested locally".'
    after: 'Evidence is refused unless it names a command or an artifact, and the recorded result carries the exit code and the commit.'
  - id: dependency-not-done
    audience: ai-native-team
    title: 'Done before its dependency'
    before: 'An item is marked complete although the item it depends on was reverted last week.'
    after: 'Status is derived from events and the dependency graph, so it changes when the dependency does.'
commands: [plan, finish, doctor]
capabilities: [graph.get]
claims: [evidence-gates-done, project-status-derived, dag-validation, finish-contract, typed-outcome]
doctrines: [majordomus.project-integrity, majordomus.verification-integrity, majordomus.dag-integrity, majordomus.verify-outcomes]
use_cases: [complete-an-issue-only-with-its-evidence, plan-the-work-as-data, accept-or-refuse-finished-work]
related: [done-because-the-model-said-so, milestone-status-unreconstructable, three-roadmaps-none-of-them-true]
aliases: ['closed but not done', 'acceptance criteria ignored', 'evidence is prose']
---

## The moment

The issue is closed. Two of its four acceptance criteria were never implemented, and the
evidence field says "tested locally". Nobody lied; the issue was closed by someone who
believed the work was finished, and nothing was in a position to disagree.

## Why it happens

Closing is a state change in a tracker, and the tracker knows nothing about the repository.
The acceptance criteria are prose in a field, so checking them is a reading, and the reading
is done by the person most convinced the work is complete.

## Why a better model does not fix it

The worker was not asked to check the criteria; it was asked to do the work, and it did.
Asking a worker to also assess its own completion returns a confident assessment, which is
the input we already had.

## What it costs

The gap between recorded and actual completion is invisible until something downstream
fails. Meanwhile the plan is used to decide what to start next, so work begins on top of
foundations that were only reported as laid.

## What Majordomus does

No status is stored anywhere. `READY`, `BLOCKED`, `ACTIVE`, `VERIFY` and `DONE` are derived
from the events an issue recorded about itself and from the state of its dependencies, every
time the plan is read; a hand-written status field is an unknown key. `plan evidence` refuses
narrative — it needs a command or an artifact, and it records the result with the commit.
`plan done` refuses while any declared evidence is uncovered or a dependency is not done.
`finish` applies the same discipline to the task: nothing is written while a line of the
contract fails.

## Before and after

```text
before   tracker: closed          repository: two criteria unimplemented

after    $ majordomus plan done I0042
         refused: evidence 'behaviour_tested' is uncovered
                  evidence must name a command or an artifact
```

## What it does not do

It does not decide whether the evidence is good evidence. It refuses evidence that is not a
command or an artifact, records what the command actually returned, and will not derive
`DONE` while a declared requirement is uncovered.
