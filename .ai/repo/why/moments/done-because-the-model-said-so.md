---
schema: moment/v1
id: done-because-the-model-said-so
kind: moment
title: 'Accepting "done" because the model said so'
short_title: 'Unverified done'
hook: 'accepted "done" because the model said so, and paid for it the next morning'
summary: 'A fluent completion claim is accepted as evidence because nothing wrote down, beforehand, what would have to be true.'
status: stable
severity: high
frequency: common
weight: 40
featured: true
audiences: [engineering-lead, enterprise, ai-native-team, solo-builder]
areas: [verification]
lifecycle: [review, implementation]
tags: [verification, evidence, completion]
signals:
  - id: done-was-a-sentence
    text: 'Work was accepted this week on the strength of a worker saying it was finished.'
  - id: red-the-next-morning
    text: 'Something merged as complete turned out to be broken, and the breakage was visible all along.'
  - id: which-tests-ran
    text: 'Nobody can say which verification actually ran for the last change that was accepted.'
examples:
  - id: all-tests-pass
    audience: ai-native-team
    title: '"All tests pass"'
    before: 'The worker ran the two tests it chose; the pipeline is red in the morning and the change touched a file nobody asked about.'
    after: '`finish` refuses until the project''s own verification command has run and exited zero, and records the command, its exit code and its duration.'
  - id: status-meeting
    audience: engineering-lead
    title: 'The status that was a paragraph'
    before: 'A report says the migration is complete; a week later the second half of it is discovered untouched.'
    after: 'The outcome is a value from a closed vocabulary — completed, partial, blocked, no_match, failed — and `partial` is a thing a worker can honestly say.'
  - id: audit-asks-for-proof
    audience: enterprise
    title: 'The auditor asks what was checked'
    before: 'The only artefact is a chat log in which a machine asserted success.'
    after: 'The ledger carries the finish event with the verification command, its exit code and its duration, attached to the task and the commit.'
commands: [finish, check]
capabilities: [health.report]
responsibilities: [finish]
claims: [finish-contract, typed-outcome, reproduce-command]
doctrines: [majordomus.verification-integrity, majordomus.verify-outcomes, majordomus.define-done-first, majordomus.note-integrity]
use_cases: [accept-or-refuse-finished-work, complete-an-issue-only-with-its-evidence]
related: [feature-without-a-test, issue-says-done-tests-disagree, site-claims-nothing-proves]
aliases: ['unverified completion', 'the model said it was done', 'no evidence of done']
---

## The moment

"Done. All tests pass." You merge. In the morning the pipeline is red, the change touched a
file nobody asked about, and the "tests" that passed were the two the worker chose to run.

## Why it happens

"Done" was a word in a transcript. No one had written down, before the work started, what
would have to be true for it to be accepted — so the worker's own claim was the only
evidence, and a fluent claim is cheap. The environments studied were full of this: status
fields with free-text values, completion notes that were never written, and a documented
per-step audit trail that no gate ever checked.

## Why a better model does not fix it

A more capable model produces a more convincing completion claim. That is the wrong axis
entirely: the problem is that a claim is being used where evidence is required, and the
fix is a gate that does not read claims. Confidence and correctness are independent, and
the more fluent the worker, the less the correlation can be relied on.

## What it costs

The rework, which is the small part. The large part is the erosion of the signal: once
"done" has meant "asserted done" a few times, nobody can use the word for anything, and
every completion has to be re-checked by a person, which is the cost the workers were
supposed to remove.

## What Majordomus does

`majordomus finish` evaluates a contract, line by line, and prints each line as pass or
fail: touched files within the declared scope; the project's own verification command ran
and exited zero, with its command, exit code and duration recorded; the task record still
describes this checkout; no open question for this task is unresolved; a handover or
completion note with the required sections exists. If any line fails, nothing is written.

The outcome is a value from a closed vocabulary — `completed`, `partial`, `blocked`,
`no_match`, `failed` — not prose. `no_match` means the work was done and the thing sought
does not exist; `failed` means the work could not be done. They look alike in a chat and are
different facts. Every refusal names the command that reproduces the failing line.

## Before and after

```text
before   worker: "Done. All tests pass."     -> merged

after    majordomus finish --outcome completed --verify-command "make test"
           OK   scope_respected
           FAIL verification_ran   exit 1   [reproduce: make test]
         nothing written
```

## How to verify it

Run `finish` with a verification command that fails. Nothing is written, the failing line is
named, and the reproduce command is printed. Then fix it and run again; the ledger carries
the exit code and duration that were actually observed.

## What it does not do

It runs the verification command you give it; it does not decide which tests matter. The
regression-test requirement in the `debugging` profile is a path heuristic and says so in
its message. It does not review code.
