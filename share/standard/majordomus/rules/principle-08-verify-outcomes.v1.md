---
id: majordomus.verify-outcomes
version: 1
kind: rule
title: Verify outcomes, not activity
description: A test that ran and passed is evidence; a sentence saying it did is not.
statement: Accept a verification command's recorded exit code as evidence of completion, and accept nothing that merely describes verification.
status: active
class: advisory
depends_on: []
tags: [principle]
---

# Rationale

Activity is easy to report and impossible to check. An outcome that a command produced, with its exit code and duration recorded, is checkable by anyone later.

Delegated work is where the difference is easiest to miss. A delegated worker's or subagent's report that it is "done", "completed" or "pushed" is a description, not an outcome: it is written by the party whose work is in question, and the harness that ran the worker reports only that its turn ended. In one observed case a worker's turn ended with "waiting for the full suite", its fixes staged and never committed, while the harness reported the task as completed. A coordinator that relayed the report would have announced a delivery that did not exist.

# Required behaviour

Accept a verification command's recorded exit code as evidence of completion, and accept nothing that merely describes verification.

When work was delegated, the coordinator observes the delivery itself before relaying the worker's report or acting on it:

- the working tree is clean, or holds only what is expected;
- the commit exists: `git log -1` in the worker's checkout names it;
- for a push, `git ls-remote origin refs/heads/<branch>` returns the sha the worker reported;
- CI is read for that exact sha, not for the branch's latest run or for an earlier commit.

What was not observed is reported as not observed, never as done.

# Failure behaviour

Nothing decides this rule directly, so a violation is reported by nobody; it is the rules that depend on it that decide what a machine can decide and stop or warn accordingly.

# Verification

The enforced rules whose `depends_on` names this one carry the validators, the tests and the CI wiring. `majordomus doctrine list` shows them.
