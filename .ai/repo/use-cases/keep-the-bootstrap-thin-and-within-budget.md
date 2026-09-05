---
id: keep-the-bootstrap-thin-and-within-budget
kind: use-case
title: 'Keep every provider bootstrap thin, generated and within budget'
summary: 'Regenerate AGENTS.md, CLAUDE.md and the other provider files from one policy and prove they are stamped, point at the layer, stay under the line budget and resolve every reference.'
category: drift
status: active
target: guaranteed
weight: 42
actors: [maintainer]
difficulty: basic
commands: [update, doctor, watch]
doctrines: [majordomus.bootstrap-integrity, majordomus.projection-integrity, majordomus.context-budget]
claims: [projection-generation, projection-fingerprint, bootstrap-chain, context-budget, pointer-integrity, no-counts-in-context, no-silent-overwrite]
responsibilities: [projection, doctor]
applications: [repository-opened-in-ai-clients, repository-with-authored-governance]
scenario:
  setup: installed-wired
  given:
    - 'installed, and the two enforcements the policy declares are actually in place as hooks'
  steps:
    - id: generate
      run: ['update']
      note: 'every projection the policy names, from the one policy, deterministically'
      expect:
        exit: 0
        stdout_contains: ['AGENTS.md', 'CLAUDE.md', 'each carries its own stamp']
    - id: prove
      run: ['doctor']
      note: 'each projection matches its stamp, points at .ai/README.md, carries no rule of its own, stays under budget, and every reference in it resolves'
      expect:
        exit: 0
        stdout_contains: ['^OK   projection', '^OK   bootstrap', '^OK   budget', '^OK   links', '^OK   counts', 'doctor: 0 failure']
    - id: quiet
      run: ['watch']
      note: 'nothing has drifted since the generation'
      expect:
        exit: 0
        stdout_contains: ['0 drift finding']
  then:
    - 'each generated file carries the policy hash and its own content hash'
    - 'the always-loaded file is under the policy’s line budget'
    - 'a hardcoded count or a dangling reference in it would have failed doctor'
---

# Situation

Every AI client loads one instruction file before it does anything. If that file grows, restates rules, or drifts from the policy, every session starts from something stale, and the cost is paid on every turn. If the tool overwrote it silently, somebody’s hand edit would vanish.

# What you run

- `update`: renders every provider bootstrap the policy declares, from the policy, with a stamp
- `doctor`: the projection matches its stamp, is a bootstrap and not a rulebook, is within the line budget, references resolve, no counts are hardcoded
- `watch`: drift since the last generation, if any

# Outcome

The bootstrap is a thin pointer at the layer, generated and checked, and the layer is where the rules live. A hand edit is detected rather than overwritten.
