---
schema: moment/v1
id: generated-artifacts-stale
kind: moment
title: 'Green tests over stale generated files'
short_title: 'Stale artifacts, green build'
hook: 'had a green pipeline over generated files that no longer matched their sources'
summary: 'Tests exercise the code and say nothing about the committed outputs derived from it, so the build is green and the artifacts are wrong.'
status: stable
severity: medium
frequency: common
weight: 270
audiences: [platform-team, ai-native-team, enterprise, engineering-lead]
areas: [verification, documentation]
lifecycle: [implementation, review]
tags: [generation, drift, ci, artifacts]
signals:
  - id: forgot-to-regenerate
    text: 'A change landed without regenerating the files derived from it.'
  - id: green-but-stale
    text: 'The pipeline is green and a committed generated file is out of date.'
  - id: regeneration-is-manual
    text: 'Regenerating derived files is a step somebody has to remember.'
examples:
  - id: forgot-the-step
    audience: ai-native-team
    title: 'A worker that changed the source only'
    before: 'A worker changes a declaration, the tests pass, and the committed reference derived from it is now a description of the previous version.'
    after: '`generate --check` names every artifact that differs from what its source produces, and CI runs it.'
  - id: two-stage-derivation
    audience: platform-team
    title: 'A derivation with an order'
    before: 'Regeneration is done in the wrong order, so one output is built from a stale input and looks current.'
    after: 'The derivation graph is written down once, run in dependency order, and proved by re-running it and comparing.'
  - id: reviewed-artifact
    audience: enterprise
    title: 'Reviewing an artifact that was not rebuilt'
    before: 'A generated document is reviewed and approved while the source it claims to describe has moved.'
    after: 'The artifact is regenerated in CI and the tree is refused if it differs; approval applies to something current by construction.'
commands: [doctor, watch]
capabilities: [capabilities.list, repository.info]
claims: [generated-projections-checked, derivation-one-graph, projection-fingerprint, site-registry-dataset]
doctrines: [project.derived-files-regenerated, project.derived-once, majordomus.projection-integrity, project.interfaces-are-projections]
use_cases: [gate-ci-on-the-tool-itself, find-out-what-drifted, extend-what-the-executable-serves]
related: [api-changed-contract-did-not, policy-changed-projection-stale, site-claims-nothing-proves]
aliases: ['stale derived files', 'forgot to regenerate', 'generated drift']
---

## The moment

Everything is green. The tests pass, the linter is quiet, and four committed files that are
generated from the code describe the code as it was before the last change. Nothing in the
pipeline looks at them, because they are not code and they are not tests.

## Why it happens

Generated files that are committed occupy an awkward category: they are reviewed like
source and produced like output. Regenerating them is a step, and a step that is not run by
the same thing that runs the tests will be skipped by whoever is in a hurry — which,
increasingly, is a worker that was asked to change the source and did exactly that.

## Why a better model does not fix it

Regeneration is a build step, not a judgement. A worker that did not run it did not fail to
reason; it was never told, and telling it every time is the manual synchronisation the
generation was supposed to remove.

## What it costs

Reviewers read a stale artifact and approve it. Consumers of the artifact — a site, a
client generator, another tool — serve last month's truth. And the tree is no longer
reproducible: running the generator produces a diff, which everyone learns to ignore.

## What Majordomus does

The derivation graph is written down once, in dependency order, so a person and CI run the
same thing in the same sequence. Every committed derived artifact has a check that
regenerates it and compares: `generate --check` names each file that differs or is missing
and exits non-zero, and the site's data has the same check against the hash of its inputs.
Running the whole derivation twice changes nothing, which is what the check compares
against.

## Before and after

```text
before   tests: green      docs/generated/*.json: from two commits ago

after    $ scripts/derive-check
         stale: docs/generated/openapi.json (differs)
                site/data/registry/registry.json (differs)
         exit 10
```

## What it does not do

It does not decide what should be generated, and it will not regenerate silently during a
check — a check that writes is not a check. It names what is stale and the command that
fixes it.
