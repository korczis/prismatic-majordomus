---
schema: feature/v1
id: finish-contract
kind: feature
title: Done is a contract, evaluated line by line and refused when unmet
short_title: Finish contract
headline: A worker does not get to define its own successful completion: every line of the finish contract is printed pass or fail, the task is not accepted until all of them pass, and every time it was claimed done and was not is on the record.
summary: A task starts with a declared scope and a profile; check reports whether the task is consistent with policy, scope and state; finish evaluates the policy's contract — scope respected, verification ran, state updated, no open blockers, a note present — and refuses with the reproducing command when any line fails.
status: stable
weight: 80
featured: true
areas: [verification]
commands: [start, check, finish, question]
rules: [majordomus.verification-integrity, majordomus.define-done-first, majordomus.verify-outcomes, majordomus.blocker-resolution, majordomus.note-integrity, majordomus.state-consistency, project.finding-carries-reproduce]
docs: [docs/CLI.md, docs/DESIGN.md]
claims: [finish-contract, finish-refusal-is-recorded, typed-outcome, open-question-gate, consistency-check, reproduce-command, exit-code-contract, blocker-store, blocker-survives-handover]
use_cases: [accept-or-refuse-finished-work, block-acceptance-on-an-open-question, carry-a-blocker-across-a-handover]
related: [doctrine, coordination]
tags: [verification, contract]
---

## What it does

The outcome of a task is a typed field with a closed vocabulary, never a sentence.
`majordomus finish --outcome completed` runs the verification command the profile requires,
walks every line the policy selects, prints each as pass or fail with the command that
reproduces a failure, and accepts nothing when any line fails; the task stays open. An open
question that names the task blocks acceptance until it is resolved, and a blocker survives
a handover rather than being lost with the conversation.

`no_match` and `failed` are different facts: the thing sought does not exist, or the work
could not be done. A supervisor that cannot tell them apart cannot decide whether to retry,
escalate or accept, so the field decides and prose never does.

Every refusal is recorded. Each refused `finish` appends a `task.refused` line to the ledger
with the outcome that was claimed, how many contract lines were unmet and which doctrines
refused, and `majordomus history --event task.refused` lists them. A task accepted after
four refusals and one accepted first time used to leave the same record; now the four false
claims of *done* are there, in order, before the one that held. That is what makes the
contract's value countable rather than asserted: each refusal later satisfied is a false
*done* the tool caught. `finish --check` asks without claiming, and records nothing.

## What it does not do

It does not prevent a worker from touching a file outside its scope while it works; it
detects the file at check and at finish and refuses to accept the work. The regression-test
line is a path heuristic and says so. Nothing here measures tokens or cost.
