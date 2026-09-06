---
schema: moment/v1
id: three-copies-of-one-explanation
kind: moment
title: 'Three copies of one explanation, and no way to tell which is current'
short_title: 'Duplicated explanation'
hook: 'found the same thing explained three times, slightly differently'
summary: 'Every hand-maintained copy of a fact drifts on its own schedule, and a reader cannot tell which copy is the current one.'
status: stable
severity: medium
frequency: constant
weight: 320
audiences: [platform-team, agency, open-source-maintainer, ai-native-team]
areas: [documentation]
lifecycle: [maintenance, onboarding]
tags: [documentation, duplication, drift, generation]
signals:
  - id: same-thing-thrice
    text: 'The same behaviour is explained in more than one document, in different words.'
  - id: which-is-current
    text: 'A reader cannot tell which of several explanations is the current one.'
  - id: update-all-copies
    text: 'Changing a behaviour means remembering to update several documents.'
examples:
  - id: readme-docs-site
    audience: platform-team
    title: 'README, reference, site'
    before: 'One behaviour is described in the README, in a reference document and on a page; two of the three are out of date and none says so.'
    after: 'One canonical statement; the others are projections of it, regenerated and drift-checked.'
  - id: onboarding-doc
    audience: agency
    title: 'The onboarding document'
    before: 'Every engagement produces a fresh onboarding document that duplicates the repository''s own conventions and then diverges from them.'
    after: 'Conventions live in scoped context documents in the repository, and the onboarding path is a pointer to them.'
  - id: worker-picks-one
    audience: ai-native-team
    title: 'A worker given all three'
    before: 'A worker is pointed at documentation containing three descriptions and has to guess which to follow.'
    after: 'There is one description, and its provenance is stated on the page it appears on.'
commands: [doctor, context, adr]
capabilities: [objects.list, objects.search]
claims: [projection-generation, context-documents, generated-projections-checked, derivation-one-graph]
doctrines: [project.context-locality, project.derived-files-regenerated, project.no-counts-in-prose, majordomus.context-integrity]
use_cases: [document-every-directory-of-the-layer, trace-a-change-to-the-context-it-affects]
related: [policy-changed-projection-stale, generated-artifacts-stale, three-roadmaps-none-of-them-true]
aliases: ['documentation duplication', 'copy-paste docs', 'which document is true']
---

## The moment

The retention behaviour is explained in the README, in a reference document and on a page of
the site. The three descriptions differ. Two of them were true once. There is no marking on
any of them to say which.

## Why it happens

Writing a second explanation is easier than finding and linking the first, especially when
the first is in a document with a different audience. Each copy is created reasonably, and
from then on the maintenance cost is multiplied while the maintenance attention is not.

## Why a better model does not fix it

A worker asked to update the documentation updates the copy it was shown. It cannot know
that two others exist. Asking it to search for duplicates first is asking inference to do a
job that a derivation would do exactly.

## What it costs

Readers act on stale copies. Contributors update one and are corrected. And the accumulated
divergence makes any single document untrustworthy, so people ask a person instead — which
is the cost the documentation existed to avoid.

## What Majordomus does

Each fact has one canonical home, and everything else that shows it is a projection with a
check. The layer's scoped documents own their directories; the policy owns the rules; the
capability declarations own the interface; the plan owns the status. The site, the reference
documents and the provider bootstraps are generated from those, and a generated file that
differs from what its source produces fails the check. Where prose would state a number, the
rule is to state the command that computes it.

## Before and after

```text
before   README.md            "records are kept for 90 days"
         docs/RETENTION.md    "records are kept for 30 days"
         site page            "records are kept indefinitely"

after    policy.yaml          ledger.retention_max_lines: 5000
         everything else      generated from it, and drift-checked
```

## What it does not do

It does not detect that two hand-written paragraphs say the same thing. It removes the need
for the second paragraph by making the places that would have held it into outputs.
