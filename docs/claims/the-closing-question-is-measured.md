# Whether a task added debt, left a branch or a worktree behind, or grew the backlog is answered by the done invariant from git, the debt baselines and the recorded forge observation, measured from where the task started; a reading that was not taken is unknown, never a pass

## What it means

When a person asks at the end of a session whether everything is done, whether any debt was
added, and whether there are more branches, worktrees and pull requests than at the start,
the answer is a reading the tool takes. A worker reports that reading. It does not answer
from what it remembers doing, and a question nothing could read is `unknown`.

## How it works

The done invariant (`apps/majordomus-cli/src/gates/done.rs`) carries three closing
questions, and `apps/majordomus-cli/src/gates/closing.rs` takes one reading for each,
measured from where the task started:

- `no-new-debt` counts the entries of every `.ai/repo/*-baseline.txt` at the commit the
  task started at and in the working tree. A baseline that gained an entry makes the
  question `fail`, committed or not; one that appears for the first time is named and is
  not growth.
- `nothing-accumulated` reads the first line of each local branch's reflog and of each
  linked worktree's `HEAD` reflog, which is when git created it. What was created since
  the task started and still exists makes the question `queued` and is named.
- `backlog-not-grown` reads the recorded forge observation and numbers the pull requests
  opened since the task started that are still open.

Each reading is a result of its own. One that fails does not take the others with it, and
its question says why it is `unknown`.

## How to see it

`majordomus check` names every question of the invariant that is not answered as
`id=status` on its `done` line; the evidence behind each is in the `questions` of
`gates.completion`, over the command line, HTTP, MCP and the Cockpit alike.

`test/cases/1037_the_closing_question_is_measured.sh` adds an entry to a baseline and sees
`fail` with the numbers, creates a branch and a worktree and sees them owed and named until
both are gone, and breaks the task's starting commit to see `unknown` where a pass would
have been a lie.

## What it does not cover

It does not say there are fewer branches, worktrees or pull requests than at the start:
what was removed in the meantime leaves no record, so the readings name what was added and
remains, with the totals now. It does not say who created a branch, because git does not;
the readings are the repository's. It removes nothing and refuses no `finish`. The backlog
reading is as old as the last `majordomus prs refresh`, and says when that was.

Two readings are coarser than the question. Debt is counted in entries, so one entry taken
out and another put in reads as no change; only growth is seen. And a branch or worktree is
dated by the first line its reflog still holds, so one whose older entries git has expired
— thirty days for commits a rewrite left unreachable — reads as younger than it is, and can
be named as created since the task started when it was not. Both err towards asking again,
never towards a pass that was not earned.

## Why it exists

On 2026-10-10 one session was asked the closing question seven times and answered seven
times from memory, each time describing only what it had touched itself. Measured on the
eighth, the repository had three more open pull requests and twenty-one more worktrees than
the answers implied. A worker's memory holds what the worker did, and the worker is the one
party with a reason to round up (`project.a-closing-question-is-answered-by-measurement`).
