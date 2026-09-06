---
schema: moment/v1
id: policy-changed-projection-stale
kind: moment
title: 'The policy changed and its four copies did not'
short_title: 'Stale projections'
hook: 'changed a policy in one place and found the old one still in force in three others'
summary: 'A decision is updated at its source and the generated copies keep serving the previous version, with nothing reporting the difference.'
status: stable
severity: high
frequency: common
weight: 170
audiences: [platform-team, enterprise, ai-native-team, engineering-lead]
areas: [governance, documentation]
lifecycle: [maintenance]
tags: [policy, projection, drift, generation]
signals:
  - id: updated-one-copy
    text: 'A rule or setting was changed in one place this month and a stale copy of it is still being used.'
  - id: no-staleness-signal
    text: 'Nothing reports when a generated file no longer matches what it was generated from.'
  - id: regenerate-by-memory
    text: 'Regenerating the derived files depends on somebody remembering to.'
examples:
  - id: checkpoint-interval
    audience: platform-team
    title: 'The interval that changed in one file'
    before: 'The checkpoint interval is raised in the policy; the instruction files keep telling every worker the old number.'
    after: '`doctor` compares each projection with the stamp of the policy it was generated from and fails on a mismatch.'
  - id: control-of-record
    audience: enterprise
    title: 'The control of record'
    before: 'The approved policy and the file the workers actually read have been different for two quarters and nobody could have known.'
    after: 'The projection carries the policy hash and its own content hash; a difference is a failing check, not a discovery.'
  - id: ci-catches-it
    audience: ai-native-team
    title: 'Caught by the pipeline instead of by a reader'
    before: 'A stale generated file is noticed when somebody reads it closely, which is rarely.'
    after: 'CI regenerates and refuses a tree in which any derived artifact differs from its source.'
commands: [update, doctor, watch]
capabilities: [repository.info]
claims: [projection-fingerprint, projection-generation, generated-projections-checked, no-silent-overwrite]
doctrines: [majordomus.projection-integrity, project.derived-files-regenerated, majordomus.policy-integrity, majordomus.policy-completeness]
use_cases: [trust-the-policy-before-reading-it, find-out-what-drifted, gate-ci-on-the-tool-itself]
related: [two-rulebooks-one-repository, generated-artifacts-stale, three-copies-of-one-explanation]
aliases: ['stale generated file', 'policy drift', 'derived file out of date']
---

## The moment

The policy was updated in March. The four files generated from it still carry the February
version, because regenerating them was a step somebody had to remember, and nothing said
they were stale.

## Why it happens

Generation without a staleness check is a one-shot copy. The moment the source changes, the
copies become claims about a past state, and they look exactly like claims about the current
one. Nothing in the file says which generation produced it or from what.

## Why a better model does not fix it

The worker reads the file it is given and applies it faithfully. It has no way to know the
file is a stale projection of something else, because staleness is a property of the pair,
and the worker only ever sees one half.

## What it costs

Workers follow a superseded policy, and the failure surfaces as behaviour nobody asked for,
weeks after the change that caused it. Tracing it back means discovering that the source of
truth was never the source anything read.

## What Majordomus does

Every generated projection is stamped with the hash of the policy it came from and the hash
of its own content. `doctor` and `watch` compare both: a source that moved on, or a file
that was edited by hand, is a named failing check. `update` regenerates deterministically
and refuses to overwrite a hand edit until the diff has been seen. The same discipline
covers every derived artifact the repository commits, and CI refuses a tree in which any of
them differs from what its source produces.

## Before and after

```text
before   policy.yaml  (March)      CLAUDE.md, AGENTS.md, ... (February)

after    $ majordomus doctor
         FAIL projection  CLAUDE.md — generated from policy 3b0d79b4, current is 9f21c0aa
                          [reproduce: majordomus update --diff CLAUDE.md]
```

## What it does not do

It does not decide whether the change should propagate; regeneration is explicit and its
diff is reviewable. It stamps and compares — it does not merge a hand edit back into the
source.
