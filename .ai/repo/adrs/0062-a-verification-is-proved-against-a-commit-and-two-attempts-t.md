---
schema: adr/v1
id: adr-0062
kind: adr
title: A verification is proved against a commit, and two attempts that disagree are not proof
status: proposed
date: 2026-09-14
tags:
  - evidence
  - verification
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0041
---

# 62. A verification is proved against a commit, and two attempts that disagree are not proof

## Context

`majordomus finish --verify-command` runs the project's own verification and records, in the
`task.finished` event, exactly `{"command": …, "exit": …, "seconds": …}`. It runs in `$MJ_ROOT` — the
working tree, as it stands, with whatever is in it — and it never reaches `.ai/repo/evidence/ledger.json`.
ADR 0041 had already decided what it takes to remember one run: the outcome, the duration, the commit it
ran against, whether the tree was clean, the digest of the source at the time, the timestamp, the origin
and the command that reproduces it, and *"a result with no commit is an anonymous green and is refused at
the point of recording"*. The finish contract records anonymous greens.

On 2026-09-14 a session in `catharsis-as-a-service` made the difference visible. Three workers shared one
checkout of one branch. `npm run validate` passed in that tree — and proved nothing about any commit,
because the tree carried two other sessions' uncommitted files, one of which (a rendered preview manifest)
was itself the difference between a red gate and a green one. The run that proved the commit was a
different run: `git archive HEAD` unpacked into a scratch directory, the gate executed there, 15 of 15
steps and 110 of 110 browser tests. Same command, same machine, same minute; one of the two was evidence
about a commit and the other was evidence about a desk.

The same session showed the second half. The browser suite against the deployed site failed once — 1
failed, 109 passed — and passed twice, 110 of 110, either side of it. A record that keeps one exit code
would have said `failing` or `proven` depending on which run it happened to keep, and both would have been
true statements about a run and false statements about the software.

## Decision

**A verification names what it ran against, and `worktree` is not a commit.** An execution carries
`source: worktree | export`. An `export` is the tree of a commit, materialised from that commit and
nothing else; a `worktree` run carries the tree state ADR 0041 already records, and a `worktree` run over
a dirty tree proves nothing about the commit it sits on. This is the distinction the existing
`working_tree: clean | dirty | unknown` field gestures at without being able to act on: clean is an
observation about a moment, an export is a statement about a commit.

**`finish --verify-command` records an execution, in the evidence ledger, with the provenance of ADR
0041.** One store. The `task.finished` event keeps its summary for readers of the ledger, and the
execution it summarises is a record that can be joined, aged and ratcheted like every other.

**Attempts are counted, and attempts that disagree are their own outcome.** One execution may carry more
than one attempt of the same command at the same commit and the same digest. All pass: `proven`. All
fail: `failing`. Mixed: `flaky` — which never proves anything and is not a failure of the tree either.
This is the smallest change that makes intermittency visible, and it keeps ADR 0041's refusal of a run
history: the ledger still holds the latest execution per identity, and the attempt count lives inside it.

**`finish --outcome completed` refuses on `flaky`.** A verification that disagrees with itself is not a
completed task; the outcome is `partial` and the note says which attempt failed, or the work is fixed.
The refusal is the point at which flakiness costs someone something, which is the only way it gets fixed.

## Alternatives rejected

**Re-run until green, and record the green.** It manufactures the green. Every flaky suite ever tolerated
was tolerated this way.

**Keep a full run history and judge flakiness statistically.** ADR 0041 refused the history and the
refusal still holds — a history is a build log with ambitions. An attempt counter inside the latest
execution answers the one question that matters at the moment of recording, and answers nothing else.

**Always verify against an export.** The export costs a second full run of the gate, which for the
repository that motivated this is minutes, and most verifications during a task are not claims about a
commit. The commit-proving run is the one that must be an export: the one `finish` records.

**Infer the export from `working_tree: clean`.** A clean tree at the moment of the run is not the same
statement as the commit's own tree, and the difference is exactly the case that caused this — untracked
files are invisible to that flag and were the deciding input.

## Consequences

An export primitive is needed and `majordomus archive` is not it: `archive` snapshots the git *index* for
distribution, this needs the tree of a named commit, materialised somewhere disposable. The disk that
costs is bounded by the same predicate as every other build output (`project.accumulation-is-measured`).

A verification becomes slower at exactly one point in the lifecycle, and the tool now knows the difference
between a verification and a habit.

`finish` gains a refusal, which means a task can now be blocked by its own flakiness. That is the intended
cost.
