---
schema: moment/v1
id: site-claims-nothing-proves
kind: moment
title: 'A public page promising something nothing tests'
short_title: 'Marketing ahead of behaviour'
hook: 'published a page that promised more than any test could support'
summary: 'Public material is written by a different act from the code, so it drifts ahead of the behaviour and nothing brings it back.'
status: stable
severity: high
frequency: common
weight: 330
audiences: [open-source-maintainer, platform-team, enterprise, engineering-lead]
areas: [documentation, verification]
lifecycle: [review, operations]
tags: [claims, marketing, evidence, trust]
signals:
  - id: page-ahead-of-code
    text: 'A public page describes a capability the code does not fully have.'
  - id: no-status-on-claims
    text: 'Nothing on a public page distinguishes what is guaranteed from what is intended.'
  - id: written-separately
    text: 'Public material is written separately from the code and reconciled by nobody.'
examples:
  - id: launch-page
    audience: open-source-maintainer
    title: 'The page written before the feature'
    before: 'A page describes the capability as it was designed; the shipped version is narrower and the page is never revised.'
    after: 'Every capability sentence is a claim with a status, an implementation and a test, and the page renders that status.'
  - id: procurement-reads-it
    audience: enterprise
    title: 'Somebody procures on the strength of it'
    before: 'A published guarantee is relied on in a decision, and it turns out to be an aspiration.'
    after: '`guaranteed`, `advisory`, `planned` and `rejected` are distinct statuses shown on the page, and a guaranteed claim without a test fails the build.'
  - id: rejected-is-published
    audience: platform-team
    title: 'Publishing what was rejected'
    before: 'An approach that was considered and refused is invisible, so it is proposed again by every reader.'
    after: 'Rejected is a status that is published with its reasoning, not an absence.'
commands: [doctor, usecase]
capabilities: [health.report, objects.list]
responsibilities: [doctor, projection]
claims: [use-case-evidence, use-case-coverage, site-registry-dataset, generated-projections-checked, reproduce-command]
doctrines: [project.no-claim-without-test, project.use-case-evidence, majordomus.use-case-coverage, project.derived-files-regenerated]
use_cases: [add-a-use-case-and-prove-it, gate-ci-on-the-tool-itself, prove-a-rule-is-enforced]
related: [feature-without-a-test, done-because-the-model-said-so, generated-artifacts-stale]
aliases: ['overclaiming', 'marketing drift', 'unproven guarantee']
---

## The moment

The page says the system does something. It nearly does. The gap is the kind that a
knowledgeable reader would call a difference of emphasis and a user would call a bug, and
it exists because the page was written from the design and the code was written from the
constraints.

## Why it happens

Public material is written once, early, when the intended behaviour is the only behaviour
there is. Shipping narrows the design; the narrowing is recorded in commits and tests, and
in nothing that anyone reconciles against the page. Nothing on the page marks which
sentences are load-bearing.

## Why a better model does not fix it

A worker asked to write the page from the design writes an accurate description of the
design. The failure is that the page is not derived from anything that changes when the
behaviour changes.

## What it costs

Trust, which is expensive to regain and is spent by exactly the readers who mattered most —
the ones who relied on the sentence. Internally it costs the same as any stale document,
plus an argument about whether the page was wrong or the implementation was.

## What Majordomus does

Every capability sentence is a claim object with a status — guaranteed, advisory, planned or
rejected — the file that implements it and the behavioural case that proves it. The public
pages render those objects rather than restating them, so a claim cannot appear on a page
with a status it does not have. A guaranteed claim without a real implementation and a real
test fails the check. Rejected is a published status with its reasoning, so an idea that was
considered and refused stays refused instead of being proposed again.

## Before and after

```text
before   page: "Majordomus verifies completion."      code: it verifies what you name.

after    claim finish-contract   guaranteed  lib/finish.sh  test/cases/06_finish.sh
         claim cost-per-outcome  planned     -              -
         (the page renders the status; a guaranteed row with '-' fails the build)
```

## What it does not do

It does not review the wording of a page, and a badly worded true claim is still badly
worded. It refuses the pairing of a strong status with no evidence.
