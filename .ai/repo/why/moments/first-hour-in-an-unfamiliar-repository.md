---
schema: moment/v1
id: first-hour-in-an-unfamiliar-repository
kind: moment
title: 'The first hour in a repository nobody can explain quickly'
short_title: 'Onboarding cost'
hook: 'spent the first hour in an unfamiliar repository working out what was normal here'
summary: 'What is conventional in a codebase is knowledge held by its regulars, so every arrival — human or machine — pays for it again.'
status: stable
severity: medium
frequency: constant
weight: 340
audiences: [agency, open-source-maintainer, ai-native-team, enterprise]
areas: [context]
lifecycle: [onboarding]
tags: [onboarding, context, conventions, adoption]
signals:
  - id: ask-a-regular
    text: 'Getting productive in this repository requires asking somebody who already knows it.'
  - id: conventions-undocumented
    text: 'What is conventional here is not written down anywhere a newcomer would find.'
  - id: every-arrival-pays
    text: 'Every new person or worker repeats the same discovery.'
examples:
  - id: new-account
    audience: agency
    title: 'A new account, a new codebase'
    before: 'A consultant''s first day is spent inferring conventions from the code, and the inferences are wrong in the ways that matter.'
    after: 'The layer is one directory with a manifest; `context resolve` composes the chain for whichever path they are about to touch.'
  - id: drive-by-contributor
    audience: open-source-maintainer
    title: 'A drive-by contribution'
    before: 'A contributor writes a reasonable patch in the project''s least favourite style, and the maintainer explains it in review.'
    after: 'The conventions are scoped documents in the repository, discoverable from the path being changed.'
  - id: worker-onboarding
    audience: ai-native-team
    title: 'A worker with no local knowledge'
    before: 'A session is pointed at a repository and asked to be useful; it reads whatever it happens to open.'
    after: 'It is told to read the layer''s protocol and resolve the context for the path, and the resolution is deterministic.'
commands: [context, init, rules]
capabilities: [objects.get, objects.list, repository.info]
claims: [ai-layer-manifest, context-documents, bootstrap-chain, context-coverage, legacy-migration]
doctrines: [majordomus.context-integrity, majordomus.ai-layout-integrity, project.context-locality, majordomus.minimum-sufficient-context]
use_cases: [adopt-an-existing-repository, document-every-directory-of-the-layer, read-only-the-context-that-fits]
related: [re-explaining-context, rule-in-a-readme-nobody-loaded, contribution-that-could-not-have-known]
aliases: ['onboarding time', 'ramp-up cost', 'unfamiliar codebase']
---

## The moment

A capable engineer, or a capable worker, is pointed at a repository and asked to change
something small. The change takes ten minutes. Working out what is normal here — the
layout, the conventions, the things that look wrong and are deliberate — takes the rest of
the morning.

## Why it happens

Conventions are the last thing anybody writes down, because to the people who hold them
they are not knowledge, they are just how things are. What does get written is a README
about the product. The gap between the two is exactly the gap a newcomer falls into.

## Why a better model does not fix it

An unfamiliar repository is an information problem, not a reasoning one. A stronger worker
infers conventions faster and infers the same wrong ones, because the evidence in the code
is genuinely ambiguous — that is why the convention had to be a convention.

## What it costs

An hour to a day per arrival, multiplied by how often arrivals happen — which, with
disposable sessions, is now several times a day rather than a few times a year. And the
inferences that were wrong are paid a second time, in review.

## What Majordomus does

The layer is one directory with a manifest that names every section, readable by a person
with no tool installed. Its protocol is stated once: read the manifest, load the sections
the task needs, resolve the rules and their dependencies, never load the local half. Each
directory carries a context document that adds to its ancestors, so what governs a path is
composed for that path rather than searched for. Adopting an existing repository is a
supported starting point: the existing instruction files are the input to the first policy
rather than something to be replaced.

## Before and after

```text
before   "read the README, then ask me"

after    $ majordomus context resolve lib/payments
         .ai/README.md            the protocol of the layer
         .ai/repo/README.md       what is canonical here
         lib/payments/README.md   amounts are integers in minor units, and why
```

## What it does not do

It does not write the conventions for you, and an empty layer explains nothing. It makes the
place they belong obvious and the act of finding them deterministic.
