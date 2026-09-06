---
schema: moment/v1
id: who-did-this-and-under-what-policy
kind: moment
title: 'A change whose provenance cannot be reconstructed'
short_title: 'No provenance'
hook: 'was asked who made a change and under what policy, and had only a commit'
summary: 'Attribution stops at the commit, so what authorised a change, what verified it and what governed it are unrecoverable.'
status: stable
severity: high
frequency: common
weight: 380
audiences: [enterprise, agency, engineering-lead, platform-team]
areas: [observability, governance]
lifecycle: [operations, review]
tags: [provenance, audit, attribution, policy]
signals:
  - id: only-a-commit
    text: 'The whole record of a change is its commit message.'
  - id: which-policy-applied
    text: 'Nobody can say which version of the rules was in force when a change was made.'
  - id: human-author-machine-work
    text: 'Work produced by a worker is attributed to whoever pressed the button.'
examples:
  - id: six-months-later
    audience: enterprise
    title: 'Six months later'
    before: 'An auditor asks what authorised a change and what verified it; the artefacts are a commit and a merged pull request.'
    after: 'The commit carries its task; the task carries the objective, the scope, the profile and the outcome; the ledger carries the verification that ran.'
  - id: client-asks
    audience: agency
    title: 'The client asks what was done and why'
    before: 'Reconstructing an engagement means reading diffs and remembering conversations.'
    after: 'The events are structured and readable back per task, with the decisions and questions that governed them.'
  - id: policy-version
    audience: platform-team
    title: 'Which rules were in force'
    before: 'The rules changed twice during the project and nothing records which version any change was made under.'
    after: 'The projections carry the policy hash they were generated from, and the policy is a tracked file with a history.'
commands: [history, session, decision]
capabilities: [objects.get, objects.search]
responsibilities: [state, watch, policy]
claims: [task-commit-attribution, git-identity, history-ledger-read, session-records, decision-attribution, projection-fingerprint]
doctrines: [project.never-author-identity, majordomus.ledger-integrity, majordomus.session-records, majordomus.projection-integrity]
use_cases: [read-back-what-happened, open-and-close-a-session, keep-decisions-out-of-the-transcript]
related: [what-the-workers-did-last-night, no-record-why-this-model, code-without-an-issue]
aliases: ['audit trail', 'change provenance', 'who authorised this']
---

## The moment

"Who made this change, what authorised it, and what verified it?" The available answer is a
commit with a human author, a message written by a worker, and a merged pull request with
one approval. None of those is the answer to any of the three questions.

## Why it happens

Git records authorship of a patch, which was a good proxy while a person typed every line.
It is no longer: the author field records who ran the worker, and everything else that
matters — the objective, the boundary, the policy in force, the verification that ran — is
either in a conversation or nowhere.

## Why a better model does not fix it

There is no inference that recovers what was never recorded. Asking a model to reconstruct
provenance from a diff produces a plausible story, which in an audit is worse than nothing.

## What it costs

In a regulated setting, the ability to demonstrate a control at all. Everywhere else, the
ability to answer the most common question in incident review: what did we think we were
doing, and what checked it.

## What Majordomus does

Identity fields are computed from git and never authored: a record that names a branch, a
head, a working tree or a set of changed files had those values computed at the moment it
was written, and a worker supplying them is an error rather than an override. A commit
carries the task that produced it. The task carries its objective, its declared scope and
the profile it ran under. The ledger carries the events — started, checkpointed, decided,
handed over, finished with a typed outcome, and the verification command with its exit code
and duration. The generated instruction files carry the hash of the policy that produced
them, so which rules were in force is a tracked fact.

## Before and after

```text
before   commit 8c31f0e  "fix auth"  author: a person

after    $ majordomus history --task t-20260903193012-a4f1
         task_started   objective="split the callback validation" profile=implementation
                        scope=lib/auth,test/auth  head=1f0a3c2
         decision       "normalise before comparing"  why=...  rejected=...
         task_finished  outcome=completed  verify="make test" exit=0 1m12s  head=8c31f0e
```

## What it does not do

It does not identify which model or vendor produced a change, and it does not sign anything.
It records what this repository decided, bounded, ran and accepted, with the identity fields
computed rather than asserted.
