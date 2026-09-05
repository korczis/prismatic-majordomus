---
id: long-running-work
kind: application
title: 'Work that outlives the session doing it'
summary: 'A task spans days, several sessions, or several people, and the reasoning has to survive the gaps.'
weight: 4
status: active
fits_when:
  - 'Decisions need to be findable later by the task and commit that made them'
  - 'Progress must be quotable without reading everything that led to it'
  - 'Something unresolved should block acceptance until it is answered'
does_not_fit_when:
  - 'The work is a single sitting and nothing needs to be recalled afterwards'
  - 'You want a knowledge base; this keeps operational state, not documentation'
use_cases: [record-a-decision-before-it-is-forgotten, capture-the-prompts-that-started-the-work, hand-work-between-sessions, find-out-what-drifted, accept-or-refuse-finished-work, keep-decisions-out-of-the-transcript, open-and-close-a-session, plan-the-work-as-data, resume-from-a-prompt-asset, block-acceptance-on-an-open-question, checkpoint-long-work, complete-an-issue-only-with-its-evidence, deliver-issues-in-waves, read-back-what-happened, read-only-the-context-that-fits, carry-a-blocker-across-a-handover]
doctrines: [majordomus.adr-integrity, majordomus.prompt-capture, majordomus.ledger-integrity, majordomus.decision-records, majordomus.blocker-resolution, majordomus.task-continuity]
responsibilities: [state, handover, finish]
---

# Context

The decisions are made once and needed repeatedly. Kept in a transcript they are unfindable; kept in prose they go stale silently; kept nowhere they are made again, differently.
