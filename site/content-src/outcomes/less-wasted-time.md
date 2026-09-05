+++
title = "Less wasted time"
description = "Spend less time and money getting AI up to speed."
weight = 3
[extra]
moments = ["task-in-progress-for-three-weeks", "re-arguing-a-settled-decision", "what-the-workers-did-last-night", "three-roadmaps-none-of-them-true"]
commands = ["start", "checkpoint", "handover", "decision", "history", "watch"]
claims = ["scoped-task", "checkpoint-record", "handover-record", "decision-record", "history-ledger-read", "drift-watch"]
+++
## The situation

Most wasted time sits between two people. Someone stops, a session ends, a model is swapped,
and the next one has a chat transcript to grep instead of a place to continue from. Tasks
stay "in progress" for weeks with nobody on them. A decision settled last month is argued
again because its reason lives in a conversation nobody can find. Asking what the AI workers
did overnight means reading everything they said.

## What Majordomus changes

Work is carried forward as records the next worker can trust. One task is active per
checkout, with its branch and state read from the repository rather than typed in.
Progress is checkpointed at the interval the profile sets, a handover has required sections
and a computed identity, and the tool resolves the right prior record for this branch and
says how far the repository has moved since. Decisions and open questions are records with
reasons, superseded rather than edited. The history is readable back by task, event and
time, and drift between what was declared and what is on disk is reported with the command
that reproduces it.

The next developer or agent continues where the last one stopped, on the same branch, with
the same decisions, instead of starting the task again.

## Where the time goes

The time that disappears is the gap between "someone stopped" and "someone else continued
productively": re-reading, re-deciding, re-discovering what was already known. Majordomus
makes that gap a handover instead of a restart. It does not make any single session faster;
it stops the work between sessions from being thrown away.

## What this does not promise

Majordomus does not run the workers, schedule them or store their memory; it supervises what
they are told, what they may touch, and whether their claim of being done can be believed. A
stale task is reported, not rescued. The pages linked below say which commands and
guarantees stand behind each sentence above.
