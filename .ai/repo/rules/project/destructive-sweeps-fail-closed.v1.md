---
id: project.destructive-sweeps-fail-closed
version: 1
kind: rule
title: A sweep that deletes fails closed on what it could not read
description: Any script that selects paths for deletion by a measured predicate — age, size, staleness — treats a predicate input it could not read as false, never as an extreme value; it names what it is about to delete and the measurement behind the verdict before deleting, and it reports how many candidates it skipped for want of a readable input.
statement: A predicate that selects something for deletion is false when its input is missing; a deleting sweep prints what it will delete and the measured value that decided it before it deletes anything, and it says how many candidates it skipped because it could not read them.
status: active
class: blocking
depends_on: []
tags: [safety, tooling]

x-majordomus:
  reviewed_because: no deleting sweep in this tree is exercised by a case yet; when one is written the guard becomes a case and this declaration falls away
---

# Rationale

A session cleaned cargo build output from worktrees whose `target/debug` mtime was older
than 24 hours, to recover a full disk. It computed `age = now - mtime`. For three
worktrees `stat` returned nothing because the directory was being written at that exact
moment; the shell substituted an empty string for the missing mtime, and the arithmetic on
an empty string produced an age of roughly 496938 hours instead of an error. Three active
worktrees looked like the oldest, most stale candidates in the set and were cleaned. Only
build output was lost, and it rebuilds, but the shape of the bug is general: a script that
turns "I could not measure this" into a number treats not-knowing as the strongest possible
answer to the question a deletion asks.

The same shape recurs wherever a predicate is arithmetic over a value that can be absent —
a missing size, a missing timestamp, a command that fails silently under `set -e` inside a
pipeline that swallows its exit code. The fix is not "check `target/debug` more carefully";
it is that no sweep is allowed to let a missing input become a verdict.

# Required behaviour

A script that deletes anything selected by a measured predicate reads the input for each
candidate and treats a read that failed, returned empty, or could not be parsed as
"exclude this candidate", never as the oldest, largest, or most stale value in the set.
The predicate itself must be false when its input is missing, not merely followed by a
guard the author remembers to add. Before deleting, the sweep prints, for each candidate it
will act on, the measured value that decided it — the age, the size, the count — not just
the verdict: a wrong predicate is then visible in the output before it does any damage. The
sweep also reports how many candidates it skipped because an input could not be read, so a
silent hole in the measurement is not silent.

# Failure behaviour

Nothing in this tool refuses a sweep script mechanically; a deleting script that computes
its predicate from an unguarded read, or that deletes without printing the measurement
first, is a defect to fix on sight, not a warning to note. A reviewer refuses a new or
changed script under `scripts/` that deletes anything and does not carry the guard.

# Verification

There is no `test/cases/` case for this yet; a sweep script exercised by one would live
under `test/cases/`, named for the script, proving that a candidate with an unreadable
input is skipped and counted rather than selected. Until one exists, the rule is held by
review: any new deleting sweep under `scripts/` is read for the guard before it is trusted,
and this file is the checklist a reviewer reads it against.
