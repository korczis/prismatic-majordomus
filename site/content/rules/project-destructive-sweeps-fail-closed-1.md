+++
title = "A sweep that deletes fails closed on what it could not read"
description = "A sweep that deletes fails closed on what it could not read"
weight = 74
[extra]
kind = "rule"
slug = "project-destructive-sweeps-fail-closed-1"
identity = "project.destructive-sweeps-fail-closed@1"
status = "active"
source = ".ai/repo/rules/project/destructive-sweeps-fail-closed.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

A script that deletes anything selected by a measured predicate reads the input for each
candidate and treats a read that failed, returned empty, or could not be parsed as
"exclude this candidate", never as the oldest, largest, or most stale value in the set.
The predicate itself must be false when its input is missing, not merely followed by a
guard the author remembers to add. Before deleting, the sweep prints, for each candidate it
will act on, the measured value that decided it — the age, the size, the count — not just
the verdict: a wrong predicate is then visible in the output before it does any damage. The
sweep also reports how many candidates it skipped because an input could not be read, so a
silent hole in the measurement is not silent.

## Failure behaviour

Nothing in this tool refuses a sweep script mechanically; a deleting script that computes
its predicate from an unguarded read, or that deletes without printing the measurement
first, is a defect to fix on sight, not a warning to note. A reviewer refuses a new or
changed script under `scripts/` that deletes anything and does not carry the guard.

## Verification

`test/cases/135_session_store_recovery.sh` is the first executable instance. `majordomus
recover` selects open episodes for closure by age, which is the shape this rule is about,
and the case proves all three halves of the required behaviour against a real run: a
candidate whose last sign of life cannot be read as a timestamp, and one carrying no
identity at all, are both left open and counted in the `skipped` line rather than selected;
the measured value that decided every candidate is printed before any of them is acted on,
including the candidates nothing happens to; and `--check` writes nothing, so the plan can
be read before the sweep runs at all.

The bug the guard exists to catch bit once more while that case was being written, from the
other direction. `mj_epoch` returns non-zero on a string neither `date` can parse, and the
first draft read it as `[ -n "$last" ] && last_e="$(mj_epoch "$last")"` — an assignment
after the final `&&` is not exempt from `set -e`, so the whole command died on the first
unreadable candidate having printed one header line. A predicate that cannot measure its
input must be false; it must not be an exception either.

Sweeps under `scripts/` are still held by review, and this file is the checklist a reviewer
reads them against; `lib/recover.sh` is what a new one should be read beside.
{% endraw %}
