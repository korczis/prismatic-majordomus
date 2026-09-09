---
schema: feature/v1
id: continuity
kind: feature
title: Sessions, prompts and handovers that outlive the conversation
short_title: Continuity
headline: A session ends and the work does not: what the next worker needs is a typed record checked against git, not a transcript somebody hopes to find.
summary: Task state, checkpoints, handovers, decisions and open questions live in files outside every conversation; a session opens and closes into an immutable record; the person's prompts are captured by the provider's own hooks; and the next episode is briefed from those records, labelled by how far git has moved since.
status: stable
weight: 50
featured: true
areas: [context, coordination]
modules: [continuity]
commands: [context, checkpoint, handover, session, capture, prompt]
kinds: [session, prompt]
rules: [majordomus.handover-integrity, majordomus.session-lifecycle, majordomus.session-records, majordomus.prompt-capture, majordomus.task-continuity, majordomus.checkpoint-freshness, project.never-store-transcripts, majordomus.handovers-carry-state]
docs: [docs/CONTINUITY.md, docs/CONTEXT.md]
adrs: [adr-0009, adr-0014, adr-0015, adr-0017]
claims: [handover-record, no-transcripts, checkpoint-record, record-resolution, context-assembly, divergence-label, session-records, session-lifecycle, prompt-capture, prompt-assets, continuity-reachable]
use_cases: [hand-work-between-sessions, checkpoint-long-work, open-and-close-a-session, capture-the-prompts-that-started-the-work, let-the-provider-draw-the-episode-boundary]
cockpit: [continuity]
related: [knowledge, provenance, coordination]
tags: [sessions, handover, prompts]
---

## What it does

A handover is an append-only record whose identity fields — branch, head, working tree,
changed files — are computed from git and refused when a body tries to author them, and
whose required sections are refused when empty. The next session resolves the right one for
its worktree and branch, never a repository-wide newest note, and reads how far git has
moved since as a label rather than a guess. Checkpoints are small by policy so they can be
quoted whole into the next briefing.

A session is the episode itself: it opens when the provider says so, may span several
tasks, and closes into a record that references what the episode produced and copies none
of it. The provider's own lifecycle hooks draw that boundary, capture the person's raw
prompts into the checkout-local half of the layer, and hand the opening episode a briefing
built from what the last one left. Prompt assets in the repository are reusable framings,
rendered against the task's state.

## What it does not do

It stores no transcript and summarises none; the schemas have no field for one. It does not
hook the worker's runtime beyond the events the provider publishes, and it does not decide
what is worth remembering: the worker writes the required sections.
