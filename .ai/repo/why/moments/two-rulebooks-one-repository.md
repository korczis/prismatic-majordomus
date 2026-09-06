---
schema: moment/v1
id: two-rulebooks-one-repository
kind: moment
title: 'Two different rulebooks for one repository'
short_title: 'Two rulebooks'
hook: 'opened CLAUDE.md and AGENTS.md and found two different rulebooks for one repository'
summary: 'Each provider reads its own hand-edited file, nothing relates them, and which contract applies depends on which tool is open.'
status: stable
severity: high
frequency: common
weight: 50
featured: true
audiences: [platform-team, open-source-maintainer, ai-native-team, engineering-lead]
areas: [governance]
lifecycle: [onboarding, maintenance]
tags: [rules, providers, projection, drift]
signals:
  - id: provider-files-disagree
    text: 'The instruction files for two different assistants say different things about this repository.'
  - id: edited-in-place
    text: 'Somebody added a rule by editing a provider file directly, and only that one.'
  - id: which-file-is-true
    text: 'Nobody can say which of the instruction files is the current one.'
examples:
  - id: march-and-may
    audience: platform-team
    title: 'March in one file, May in the other'
    before: 'A rule added to CLAUDE.md in March and a different one added to AGENTS.md in May; the repository now has two operating contracts.'
    after: 'One policy generates every instruction file deterministically, each stamped with the policy hash and its own content hash.'
  - id: contributor-reads-the-wrong-one
    audience: open-source-maintainer
    title: 'The contributor read the other file'
    before: 'A contribution follows the conventions in the file its author''s assistant happened to load, and violates the ones in the other.'
    after: 'Both files are projections of the same body, so following either is following the same contract.'
  - id: new-provider
    audience: ai-native-team
    title: 'A third assistant joins'
    before: 'A third file is written by copying one of the first two, and immediately begins drifting from both.'
    after: 'Adding a provider is adding its template and its projection to the policy; the body is not copied.'
commands: [update, doctor, watch]
capabilities: [repository.info]
responsibilities: [policy, projection, doctor]
claims: [projection-generation, projection-fingerprint, no-silent-overwrite, context-budget, wiring-reconciliation]
doctrines: [majordomus.projection-integrity, majordomus.bootstrap-integrity, majordomus.policy-integrity, majordomus.context-budget]
use_cases: [trust-the-policy-before-reading-it, keep-the-bootstrap-thin-and-within-budget, find-out-what-drifted]
related: [policy-changed-projection-stale, rule-in-a-readme-nobody-loaded, enforcement-nothing-invokes]
aliases: ['CLAUDE.md and AGENTS.md disagree', 'provider instruction drift', 'two operating contracts']
---

## The moment

Someone added a rule to `CLAUDE.md` in March. Someone else added a different rule to
`AGENTS.md` in May. Today the repository has two operating contracts, and which one applies
depends on which tool happens to be open.

## Why it happens

Each AI tool reads its own file, each file is hand-edited, and nothing relates them. In a
workspace of about twenty repositories, the two files for the same repository shared between
none and a tenth of their content — not duplicates, disjoint rule sets. One always-loaded
contract oscillated between empty and about eleven hundred lines across two hundred hand
edits. Rules also decay in a second way: a hook is documented as enforcing, exists on disk,
and is dispatched by nothing. Seventeen such cases were found before this tool was designed.

## Why a better model does not fix it

The worker read its instruction file correctly. It read the wrong one, or rather it read one
of two that were both presented as authoritative. No amount of capability lets a worker
detect that a file it was never shown contradicts the file it was.

## What it costs

Two contradictory contracts do not produce two behaviours; they produce arbitrary
behaviour, because which one is loaded depends on which client someone opened. The
correction then happens in review, one violation at a time, and the reviewer's fix is
usually to edit whichever file is in front of them — which widens the gap.

## What Majordomus does

There is one canonical policy, `.ai/repo/policy.yaml`. `majordomus update` generates every
instruction file the policy names — `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or any target you
add — from the same body, deterministically, and stamps each with the policy hash and the
hash of its own content. A hand edit is detected by `doctor` and `watch`; `update` refuses to
overwrite it until you have seen the diff. The always-loaded file has a line budget with a
failing check.

`majordomus doctor` also reconciles the policy's enforcement list against what actually
runs: the path exists, is executable, and is invoked by the hook it names without its exit
code being swallowed. Declared-but-not-wired was the most common failure in the source
material; here it is a failing check, applied to this repository's own hooks first.

## Before and after

```text
before   CLAUDE.md   (hand-edited, March)
         AGENTS.md   (hand-edited, May)      disjoint rule sets

after    .ai/repo/policy.yaml -> majordomus update -> CLAUDE.md, AGENTS.md, ...
         echo "my own rule" >> CLAUDE.md && majordomus doctor
           FAIL projection  CLAUDE.md — content does not match its stamp
```

## How to verify it

Append a line to a generated instruction file and run `doctor`. The stamp no longer matches
its content and the check fails, naming the file; `update` refuses to overwrite it silently.

## What it does not do

It projects a deliberately narrow subset of the policy and lists what is not projected. It
does not merge existing hand-written instruction files; on first `update` you decide what
moves into the policy body. Providers it has no template for get the generic Markdown.
