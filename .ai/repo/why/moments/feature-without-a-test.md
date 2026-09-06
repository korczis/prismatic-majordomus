---
schema: moment/v1
id: feature-without-a-test
kind: moment
title: 'A capability the documentation promises and nothing proves'
short_title: 'Unproven claim'
hook: 'read a promise in the README that no test stood behind'
summary: 'A sentence describing what the software does is written once and never connected to anything that would fail if it stopped being true.'
status: stable
severity: high
frequency: common
weight: 240
audiences: [open-source-maintainer, platform-team, engineering-lead, enterprise]
areas: [verification, documentation]
lifecycle: [review, maintenance]
tags: [claims, tests, evidence, documentation]
signals:
  - id: claim-no-test
    text: 'The documentation states a capability and no test would fail if it disappeared.'
  - id: cannot-name-the-test
    text: 'Nobody can name the test that proves a given documented behaviour.'
  - id: aspirational-prose
    text: 'Documentation describes intended behaviour alongside actual behaviour, with nothing marking which is which.'
examples:
  - id: readme-promise
    audience: open-source-maintainer
    title: 'A README that promises'
    before: 'The README says the tool refuses invalid input; the refusal was removed in a refactor and nothing noticed.'
    after: 'Each capability sentence is a claim with a status, an implementation and a test, and a claim with no test may only be phrased as a target.'
  - id: matrix-not-prose
    audience: platform-team
    title: 'Guaranteed, advisory, planned'
    before: 'Marketing prose and engineering reality are in the same paragraph and are not distinguishable.'
    after: 'The claim carries its own status, so "guaranteed" and "planned" are different words with different obligations.'
  - id: audit-the-promises
    audience: enterprise
    title: 'Which promises are load-bearing'
    before: 'A capability relied on by a control is documented and unproven, and the gap is discovered during an audit.'
    after: 'A guaranteed claim names its implementation file and its behavioural case, and CI refuses a claim that names neither.'
commands: [doctor, usecase, doctrine]
capabilities: [health.report, objects.list]
claims: [use-case-evidence, use-case-coverage, reproduce-command, doctrine-registry]
doctrines: [project.no-claim-without-test, project.use-case-evidence, majordomus.use-case-coverage, majordomus.verify-outcomes]
use_cases: [add-a-use-case-and-prove-it, prove-a-rule-is-enforced, gate-ci-on-the-tool-itself]
related: [site-claims-nothing-proves, done-because-the-model-said-so, documented-command-no-longer-works]
aliases: ['undocumented regression', 'claims without tests', 'aspirational documentation']
---

## The moment

The README says the tool validates the input before writing. It used to. The validation was
removed during a refactor eight months ago and the sentence stayed, because nothing related
the sentence to the code.

## Why it happens

Prose and behaviour are maintained by different acts. Deleting code is a change with a
diff; deleting the sentence that described it is an act of remembering. Nobody remembers,
and no check exists, because the sentence is not a testable artefact — it is a paragraph.

## Why a better model does not fix it

A worker asked to implement against the documentation implements against a description of a
system that no longer exists, faithfully. Documentation that cannot be false is
indistinguishable from documentation that is true.

## What it costs

Users and contributors build on a promise that is not kept. Worse, the promise is used as a
specification: somebody restores the described behaviour badly, or builds a layer on top of
a guarantee that was never there.

## What Majordomus does

A capability sentence is a claim with a status — guaranteed, advisory, planned or rejected —
the file that implements it and the behavioural case that proves it. A guaranteed claim
without a real implementation and a real test fails the check, and a sentence that cannot be
backed is phrased as a target instead. Use cases go further: each names the commands and
claims it exercises and carries a scenario that is executed against the real tool, so the
example on a page is the output of a run rather than a paste.

## Before and after

```text
before   README: "validates input before writing"     (last true in January)

after    $ majordomus doctor
         FAIL claims  input-validation — status guaranteed, test '-' 
                      [reproduce: majordomus usecase coverage]
```

## What it does not do

It does not write tests, and it cannot tell whether a test is a good one. It refuses the
combination of a strong claim and no evidence, which is the state in which documentation
starts lying.
