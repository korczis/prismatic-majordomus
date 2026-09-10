---
id: know-whether-this-work-is-finished
kind: use-case
title: 'Know whether the work in this checkout is finished'
summary: 'Ask the repository you are standing in, and be told what is still owed by name.'
category: completion
status: active
target: advisory
weight: 7
actors: [agent, maintainer]
difficulty: basic
commands: [check, watch]
doctrines: [majordomus.obligation-closure, majordomus.scope-integrity, majordomus.state-consistency]
---

# Situation

The work looks done. Everything compiles, the change reads well, and the session is
about to end or be handed over. Whether it is *finished* is a different question, and
the answers to it are scattered: the tree may hold files outside the claimed scope,
the projections may no longer match what they were rendered from, the tests may have
been run before the last three edits, and the branch may exist on no remote at all.

The obligations a task declared at `start` are checked by `check`. What is not checked
is what the *class of work* owes regardless of what anyone remembered to declare — and
a task that declared nothing is told, truthfully and uselessly, that it declares nothing.

# What you run

- `check`: the task's own consistency — scope, state, doctrines, and the obligations it promised
- `watch`: whether policy, projections, state and retention still agree
- the obligation steps below: what this class of work owes whether or not it was promised

# Scenario

```yaml
mode: live
steps:
  - id: consistent
    run: ['check']
    note: 'the task is consistent with its scope, its policy and this checkout'
    expect:
      exit: 0
  - id: nothing-drifted
    run: ['watch']
    note: 'no projection, record or piece of state disagrees with what produced it'
    expect:
      exit: 0
  - id: the-change-exists
    obligation: implementation
    note: 'there is a change; an empty tree is not finished work'
  - id: the-cases-were-run
    obligation: tests
    note: 'run over these inputs, not over an earlier shape of them'
  - id: a-reader-was-considered
    obligation: docs
  - id: the-projections-are-current
    obligation: generated
  - id: committed
    obligation: commit
    note: 'settled live at HEAD; a clean tree is the evidence'
  - id: pushed
    obligation: push
    note: 'a branch whose commits reach no remote is invisible to every other worker'
then:
  - 'every outstanding item is named, with the command that would discharge it'
```

# Outcome

The verdict is `unmet`, not `fail`: nothing is broken, something is owed. Each step
that is not discharged prints what is missing and the command that discharges it, and
a step whose evidence no longer describes this tree reports `stale` rather than passing
on the strength of a run that happened three edits ago.

The judgement is the same one `check` reaches — `mj_obligation_judge` is read by both —
so a gate and the task's own contract cannot come to disagree about whether the same
tree is finished. Nothing here writes: every command in the scenario is declared
`class: read-only` in `share/commands.yaml`, and `usecase validate` refuses a live step
that is not.
