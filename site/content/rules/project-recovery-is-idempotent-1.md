+++
title = "Recovery looks before it repeats"
description = "Recovery looks before it repeats"
weight = 113
[extra]
kind = "rule"
slug = "project-recovery-is-idempotent-1"
identity = "project.recovery-is-idempotent@1"
status = "active"
source = ".ai/repo/rules/project/recovery-is-idempotent.v1.md"
+++
{% raw %}

## Rationale

The repository already knows that a worker stopping is not a rollback: an interrupted worker
leaves a working tree, real work in an unknown state owned by nobody. That rule says preserve
it. This one says what the *next* worker does with it, and the answer is not "start again",
because starting again is how one interruption becomes two problems.

An interrupted operation has an unknown position on a spectrum, and every point on it is
reachable: it may have done nothing, done half, done all of it and died before saying so, or
done all of it and reported successfully to a parent that was not listening — the last being
exactly what the terminal-focus stall of 2026-09-10 produced, where finished results sat
unconsumed while the parent looked idle. A retry that assumes the first case is wrong in the
other three, and it is wrong in an expensive direction: a duplicated commit, a file written
twice, a migration applied to a database that already had it, an identifier allocated a second
time. Undoing a duplicate is harder than not making one.

This is also where the naive fix for a stall goes wrong. Having noticed that a delegate looks
idle, the obvious move is to spawn it again. If the delegate had in fact finished, the second
run is pure waste at best and a conflicting second copy of the work at worst — and the first
result is still sitting there, still unconsumed, which was the actual defect.

## Required behaviour

**Establish before repeating.** Before retrying interrupted work, look for its effects: the
artifact it was to produce, the git state, the build and test output, the task or session
record. Retry only what can be shown not to have succeeded.

**Consume before re-running.** Where a delegate or child appears idle, first determine whether
it completed and whether a result is queued and unread. A result that already exists is taken
up; it is not recomputed. Re-running is what happens after the work is shown to have genuinely
failed, not after it merely looks quiet.

**Resume from the last deterministic state**, not from the beginning. Completed work is kept.
This is the same discipline the repository already applies to a checkpoint commit: whoever
picks it up judges each part and keeps, fixes or rejects it, rather than discarding the tree.

**Prefer resumable and idempotent operations** when arranging work in the first place, so that
the question this rule asks has a cheap answer. An operation that can safely be run twice needs
no forensics; an operation that cannot needs a marker that says whether it ran.

## Failure behaviour

Decided by review, and by the damage. Nothing can mechanically detect that a retry was about to
duplicate a side effect; what the repository can and does detect is the aftermath — a duplicate
identity in the index refuses, and `project.destructive-sweeps-fail-closed` governs the
cleanup half.

## Verification

Review, and one executable instance. `majordomus recover` is this rule made mechanical for
the record stores: it establishes what already happened before it repeats anything — a
record for the episode already existing is the normal outcome of a re-run and of a crash
between the publish and the teardown, and it keeps that record rather than writing a second
— and `test/cases/135_session_store_recovery.sh` proves it, running every subject twice and
asserting that the second run takes no action, and reconstructing an interrupted run
(the record written, the open file still there) to assert that the resume keeps what exists.

Everywhere else this is review. The evidence a recovery is expected to consult is the same
evidence the repository already keeps: `git status` and the branch's own history, the derived
artifacts, the session and task records under `.ai/`.
{% endraw %}
