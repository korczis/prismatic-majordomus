---
schema: skill/v1
id: report-verification-state
version: 1
title: Report by verification state
description: Write the outcome of a piece of work as claims each marked VERIFIED, PARTIAL or BLOCKED by the evidence actually observed, so a reader can tell what was proved from what was only done or intended.
status: active
tags: [reporting, evidence, handover]
related: [implement, repo-review]
inputs:
  - the work as it stands in the tree, with its commits and anything pushed
  - the commands that ran during the work, with their exit codes and the relevant output
outputs:
  - a report whose every claim carries one of VERIFIED, PARTIAL or BLOCKED and the evidence for it
  - the gaps, each with the command or decision that would close it
provenance:
  origin: prior-art
  ledger: import-2026-09-09#3
  decision: adapted
---

# Purpose

A report that says "done" merges three different facts: the change exists, the change was
checked, and the check showed it works. Readers act on the third and usually receive the
first. This skill makes the difference part of the report's grammar: every claim carries the
state the evidence supports, and a state without evidence is not available.

# When to use

Any report another worker or a person will act on: a handover body, a task's final report,
a pull request description, an answer to "is it finished?". The handover prompt's
`Verification` section is written this way.

# Procedure

## 1. Write the claims, not the story

List what the work claims is now true, one claim per line, each small enough to be checked
by one command: "`skills verify` exits 0 on master", not "skills are done".

## 2. Attach the evidence you observed

For each claim, the command that ran, its exit code and the line of output that decides it.
Only what ran in this session, or what a recorded execution (the evidence ledger, a CI run
you read) shows, is evidence. A command you expect to pass is not.

## 3. Mark the state

- **VERIFIED** — the evidence decides the claim, at the commit the report is about.
- **PARTIAL** — some evidence exists and a named part of the claim is not covered: another
  platform, a gate that did not run, a stale run, a surface not exercised. Name the part.
- **BLOCKED** — the claim cannot be verified now. Name what blocks it (a missing tool, a
  queue, a decision that is not yours) and what would unblock it.

When in doubt between two states, take the weaker one.

## 4. Order and close

Put BLOCKED first, then PARTIAL, then VERIFIED, so the reader meets what needs them first.
End with one line per gap: the command to run or the decision to take, and who can take it.

# Output

A table, then the gaps:

| state | claim | evidence (command, exit code, deciding output) |
|---|---|---|

No claim appears without a state, no VERIFIED claim appears without a command, and the
report's summary line counts each state rather than saying "done".
