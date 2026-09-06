---
schema: moment/v1
id: re-explaining-context
kind: moment
title: 'Re-explaining the same context to a brand-new session'
short_title: 'Re-explaining context'
hook: 're-explained the same context to a brand-new session'
summary: 'Repository knowledge that only ever existed in a conversation has to be re-transmitted by hand to every worker that follows.'
status: stable
severity: medium
frequency: constant
weight: 10
featured: true
audiences: [solo-builder, agency, ai-native-team, engineering-lead]
areas: [context]
lifecycle: [onboarding, implementation, handover]
tags: [context, sessions, continuity, handover]
signals:
  - id: same-paragraph-again
    text: 'I typed the same explanation of this repository into a fresh session again this week.'
  - id: architecture-in-chat
    text: 'Important architectural context exists mainly in previous conversations.'
  - id: warm-up-cost
    text: 'A new session needs a warm-up period before it is useful, every time.'
examples:
  - id: monday-tuesday
    audience: solo-builder
    title: 'Monday knew it, Tuesday does not'
    before: 'Monday''s session understands the module boundaries; Tuesday''s spends forty minutes rediscovering them from the source.'
    after: 'Monday''s session ends with a handover naming the objective, the state and the next action; Tuesday''s resolves it in one command.'
  - id: worker-to-worker
    audience: ai-native-team
    title: 'One worker learns it, the next does not'
    before: 'Worker A discovers a constraint the hard way; worker B, started an hour later, implements the thing the constraint forbids.'
    after: 'The constraint is a decision record with its reason, and it is in the context every later worker is assembled with.'
  - id: five-sessions-one-week
    audience: agency
    title: 'Five sessions, one client convention'
    before: 'A senior architect explains the client''s repository conventions to five different sessions in one week, from memory, slightly differently each time.'
    after: 'The convention is a context document for the directory it governs, resolved for any path in one command, and nobody re-transmits it.'
commands: [handover, check, context]
capabilities: [objects.get, objects.search]
responsibilities: [state, handover]
claims: [handover-record, no-transcripts, git-identity, divergence-label]
doctrines: [majordomus.handover-integrity, majordomus.sessions-are-workers, majordomus.handovers-carry-state, majordomus.minimum-sufficient-context]
use_cases: [hand-work-between-sessions, read-only-the-context-that-fits, resume-in-the-right-worktree]
related: [discovery-never-becomes-knowledge, decision-only-in-a-transcript, first-hour-in-an-unfamiliar-repository]
aliases: ['lost session context', 'cold start', 'context re-transmission']
---

## The moment

A task ran long, the session ended, and the next one begins with the same paragraph you
typed yesterday: which files matter, what was decided, what is still open. Fifteen minutes
later the worker is where the previous one was — if you remembered everything.

## Why it happens

The conversation was the database. Everything the worker learned lived in a transcript that
the next session cannot read, so the only way to transfer it is to re-transmit it, by hand,
from memory. Long transcripts make it worse: the state is in there somewhere, diluted by
everything that did not matter.

The environments this tool was distilled from had a session-notes directory that grew to
ten gigabytes and a hand-written runbook for recovering working state after a session ended.
Notes had dates in their filenames that disagreed with their content, and nothing checked
either against git.

## Why a better model does not fix it

A model with a larger window still starts empty. The limit is not how much it can hold but
whether anything durable exists to hand it: a session that begins with no state has nothing
to load, however much it could load. Better models make the re-explanation faster to write
and no less necessary.

## What it costs

Fifteen to forty minutes at the start of every session, paid by the most expensive person on
the project, at the moment they have the least patience for it. The second cost is worse:
the re-transmission is from memory, so it is lossy, and the parts that get dropped are the
constraints nobody thought to mention — which is exactly how the next defect gets written.

## What Majordomus does

A task's durable facts live outside every conversation, in a handful of typed files. When a
session ends, `majordomus handover` writes an append-only record whose identity fields —
branch, head, working tree, changed files — are computed from git, never authored by the
worker. The body must carry `# Objective`, `# Current State` and `# Next Action`, each
non-empty, or the record is refused.

The next session runs `majordomus handover --resolve`. It picks the most relevant record for
this worktree and branch, never a repository-wide "newest note", and labels how far git has
moved since: `exact`, `advanced`, `diverged` or `different_context`. Stale state is named,
not silently trusted. `majordomus check --explain` prints the effective policy and profile so
the worker loads what the task needs and nothing more.

## Before and after

```text
before   session 2: "so, this repository is an umbrella of ..." (15 min, from memory)

after    session 1: majordomus handover     # Objective / Current State / Next Action
         session 2: majordomus handover --resolve
                    resolved t-20260903193012-a4f1  advanced (+3 commits)
```

## How to verify it

End a session with a handover and start another on the same branch. The record comes back
with a divergence label computed from git, not asserted; move the branch on and the label
changes without anyone editing the record.

## What it does not do

It does not summarise transcripts, and it refuses to store them. It does not decide what is
worth remembering; the worker writes the three required sections. It does not hook the
worker's runtime: the worker runs `handover` because its generated instructions tell it to.
