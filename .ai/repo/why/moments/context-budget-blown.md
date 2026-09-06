---
schema: moment/v1
id: context-budget-blown
kind: moment
title: 'The instruction file that grew into a rulebook'
short_title: 'Bootstrap bloat'
hook: 'watched the always-loaded instruction file grow past a thousand lines'
summary: 'Everything important gets appended to the always-loaded file, so every session pays for every rule and reads none of them carefully.'
status: stable
severity: medium
frequency: common
weight: 120
audiences: [platform-team, ai-native-team, enterprise, engineering-lead]
areas: [context, governance]
lifecycle: [maintenance, onboarding]
tags: [context, budget, rules, bootstrap]
signals:
  - id: file-keeps-growing
    text: 'The always-loaded instruction file only ever grows, and nobody removes anything from it.'
  - id: paying-for-everything
    text: 'Every session loads rules for parts of the repository it will never touch.'
  - id: nobody-reads-it-all
    text: 'Nobody, human or machine, is confident they have read the whole instruction file.'
examples:
  - id: append-only-rulebook
    audience: platform-team
    title: 'Append-only by convention'
    before: 'Each incident adds a paragraph; nothing is ever deleted, because deleting a rule feels like removing a control.'
    after: 'The bootstrap points at the layer and is held to a line budget with a failing check; rules live where they are governed.'
  - id: cost-per-session
    audience: enterprise
    title: 'Paid on every call'
    before: 'A thousand-line preamble is billed on every session in every repository, most of it irrelevant to the task.'
    after: 'The projection is generated from a policy, its size is a checked number, and what is not projected is listed.'
  - id: ignored-in-practice
    audience: ai-native-team
    title: 'Long enough to be ignored'
    before: 'The file is long enough that both people and workers skim it, so the important rules are diluted by the unimportant ones.'
    after: '`context` assembles what this task needs within a budget, and names every section it dropped.'
commands: [update, context, doctor]
capabilities: [repository.info]
responsibilities: [policy, projection]
claims: [context-budget, no-counts-in-context, context-selection-budget, projection-generation]
doctrines: [majordomus.context-budget, majordomus.minimum-sufficient-context, project.context-locality, majordomus.bootstrap-integrity]
use_cases: [keep-the-bootstrap-thin-and-within-budget, read-only-the-context-that-fits]
related: [rule-in-a-readme-nobody-loaded, two-rulebooks-one-repository, three-copies-of-one-explanation]
aliases: ['context bloat', 'CLAUDE.md too long', 'always-loaded budget']
---

## The moment

The file every session loads before it does anything is now longer than most of the source
files it describes. It is the first thing every worker reads and the last thing anybody
edits deliberately.

## Why it happens

Appending is the only safe-feeling operation. A rule that matters gets added to the file
everyone loads, because that is the only way to be sure it is seen; nothing is ever removed,
because removal looks like weakening a control. The file therefore grows monotonically and
its signal-to-noise ratio falls monotonically with it.

## Why a better model does not fix it

A larger context window makes the file cheaper to load and no more likely to be applied. A
rule buried at line 800 among 400 others competes for attention with everything around it,
and the competition is decided by salience rather than by relevance to the current path.

## What it costs

Tokens on every session, which is measurable, and attention on every session, which is not.
The second cost is the real one: past a certain length, an instruction file stops being a
contract and becomes background texture.

## What Majordomus does

The bootstrap is generated, not written, and it is small on purpose: it says how to find the
policy and the rules, never what they are. The policy sets a hard line budget for the
always-loaded projection and a separate budget for what `majordomus context` prints; both
are failing checks, so growth is caught at the moment it happens rather than at the audit.

Everything else lives where it is governed, in the layer's scoped context documents, and is
composed for the path a worker is about to touch. `majordomus context` assembles within its
budget and names every section it dropped, so a truncated briefing is visible rather than
silent.

## Before and after

```text
before   CLAUDE.md   1,100 lines, loaded by every session

after    $ majordomus doctor
         OK budget  AGENTS.md — 46 lines, budget 150
         OK context builder — 18 lines, budget 300
```

## How to verify it

Append lines to the always-loaded projection past the budget and run `doctor`: the budget
check fails with the line count and the limit. `majordomus context` prints its own size and
what it omitted.

## What it does not do

It does not decide which rules matter, and it does not summarise a rule to make it fit. A
rule that does not fit the budget belongs in a scoped document, not in a shorter paraphrase.
