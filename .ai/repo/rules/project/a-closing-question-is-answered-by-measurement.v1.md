---
id: project.a-closing-question-is-answered-by-measurement
version: 1
kind: rule
title: Whether the work is done is answered by measurement, never by the worker's recollection
description: When a person asks whether everything is done, merged and deployed, whether any debt was added, and whether there are more pull requests, branches and worktrees than at the start, the answer is the done invariant's, read from git, the debt baselines and the recorded forge observation; a worker does not answer from what it remembers doing, and a reading that was not taken is unknown, not yes.
statement: Answer a closing question by running the measurement and reporting each question with its status and the evidence it read; never summarise from memory, never report only what you yourself touched, and never let a reading you did not take stand as a yes.
status: active
class: blocking
depends_on: [project.a-worker-that-stops-leaves-its-work-behind@2, project.never-reported-is-not-green@1, project.accumulation-is-measured@2, project.interfaces-are-projections@1]
tags: [process, integration, evidence, agents]

x-majordomus:
  tests: [test/cases/1037_the_closing_question_is_measured.sh, test/cases/131_completion_gates.sh]
---

# Rationale

On 2026-10-10 the owner asked one session the same question seven times: is everything from
this session done, can it be ended, is everything merged, was no debt added, are there
fewer pull requests than at the start, what about the branches and the worktrees, is
everything integrated and deployed.

Seven times the session answered from what it remembered doing. Each answer was true of the
session's own footprint — it had opened no pull request, its one branch and its one worktree
were gone — and each said nothing true about the repository, which is what was asked. The
eighth answer was measured: 52 pull requests open against 49 when the session began, 42 of
them conflicting with the trunk; 117 worktrees where the previous session had left 96; 225
local branches, 59 of them already reached by the trunk and 13 holding commits no remote
had; ten stashes nobody had audited; the release untagged. None of that was in any of the
seven answers, and none of it could have been: a worker's memory holds what the worker did,
not what three other sessions did in the same hour.

The repository already knew that a claim of completion has to be a reading and not a
sentence. The done invariant asked nineteen questions of a task and took every answer from
the subsystem that held the fact, and `project.never-reported-is-not-green` already said
that a verdict that never arrived is not a pass. What the invariant did not ask was the
part of the closing question that is about the repository over time: whether recorded debt
grew, whether what was created is still lying there, whether the backlog got longer. Those
were left to be answered in prose, and prose is where they were answered wrongly.

There is a second reason the worker is the wrong source, and it is not about honesty. The
worker asked "is it done?" is the one party with a reason to round up, at the moment it
most wants the conversation to end. A measurement has no such moment.

# Required behaviour

**A closing question is answered by running the measurement.** `majordomus check` names
every question of the done invariant that is not answered, as `id=status`; `gates.completion`
carries each question with its status, the evidence it read and the source that answered
it. That document is the answer. A worker reports it — the questions that pass, the ones
owed, the ones refused, the ones unknown — and adds what it knows on top, never instead.

**Three questions carry the part that is about the repository**, each read from the record
that holds the fact and measured from where the task started:

- `no-new-debt` — the entries of every debt baseline (`.ai/repo/*-baseline.txt`) at the
  commit the task started at, against the working tree. An entry added is debt the change
  created and then permitted itself; it refuses, committed or not. A baseline that appears
  for the first time records debt that was already there; it is named and is not growth.
- `nothing-accumulated` — the local branches and linked worktrees that git's reflogs say
  were created since the task started and that still exist. While one remains the question
  is owed, and the task's own branch is among them until the work has landed and been
  cleaned up.
- `backlog-not-grown` — the pull requests the recorded forge observation says were opened
  since the task started and are still open.

**The readings are the repository's, not the worker's.** Git does not record who created a
branch, so a branch another session created since the task started is counted. That is the
question a person asks at the end — did this get bigger while we worked — and "not mine" is
a remark a worker may add, with the evidence for it, after the count.

**A reading that was not taken is `unknown`.** No task record means no start to measure
from; a forge not observed since the task started cannot say what those hours brought; a
commit that is not in the repository cannot be compared with. Each says which it is. None
of them is a pass, and a worker does not turn one into a yes by explaining it.

**A total that was not recorded is not reconstructed.** What was removed since the start
leaves no record, so the readings name what was added and remains, and state the totals
now. They do not claim the difference of two totals, and neither does a worker.

**A reading is as fine as its record, and says no more.** Debt is counted in entries: one
entry taken out and another put in is not seen, only growth. A branch or a worktree is
dated by the first line its reflog still holds: where git has expired the older entries it
reads as younger than it is, and may be named as created since the task started when it was
not. Both limits lean towards a question asked again and never towards a pass.

# Failure behaviour

`gates.completion` answers; `majordomus check` reports each question that is neither `pass`
nor `exempt` on one `done` line with its status. Nothing is deleted, no branch is removed
and no baseline is rewritten on anybody's behalf: the finding is the question and its
evidence, and the remedy is the command the question carries.

A worker that answers a closing question without the reading has not answered it. The
correction is the reading, not a better sentence.

# Verification

- `apps/majordomus-cli/src/gates/closing.rs` — the three readings, each a `Result` of its
  own, with unit tests over a real repository: an entry counted at the start and now, the
  first line of a reflog, an observation older than the task refused rather than read as
  empty.
- `apps/majordomus-cli/src/gates/done.rs` — the questions and the one place a reading
  becomes an answer; its tests hold that an unread reading is `unknown`, that growth in one
  baseline is not paid for by shrinkage in another, and that with nothing behind it no
  question passes.
- `test/cases/1037_the_closing_question_is_measured.sh` — the behavioural case, in a
  disposable repository: an entry added to a baseline refuses, uncommitted or committed, and
  taking it out answers; a branch and a worktree created since the task started are owed
  and named until both are gone; a checkout that never observed the forge is `unknown`; and
  `majordomus check` names each by its id and status.
- `test/cases/131_completion_gates.sh` — the invariant as a whole: every question carries a
  source and its evidence.
