# A blocking question will keep blocking after the work is handed to a new task

## What it means

**This is not implemented.** It is published so that the gap is visible rather than assumed to be covered.

An unresolved question refuses `finish --outcome completed` for the task that opened it. Hand that work over, start a new task to continue it, and the question is still unresolved — and no longer refuses anything. The blocker is laundered by the handover.

## How it works today

`question add` writes the active task's id into the entry. `check` and `finish` match entries by that id. A new task has a new id, so the old entry matches nothing and the gate passes.

The question is not lost: `majordomus question list --all` shows it, and it stays in the file until someone resolves it. Nothing draws attention to it.

## How to see it

```bash
majordomus question add "does the legacy client still need the plain method?"
majordomus finish --outcome completed --verify-command true   # refused, correctly
majordomus handover --close < note.md
majordomus start "continue the same work" --scope lib/auth
majordomus finish --outcome completed --verify-command true   # accepted, with the question still open
majordomus question list --all                                # it is still there
```

## What it does not cover

Nothing, yet. Until it is fixed: resolve open questions before handing over, or re-open them against the new task. `question list --all` is the check a person can run.

## Why it is not fixed yet

Both obvious fixes are wrong in a case that matters.

Transferring a task's questions to whichever task resumes from its handover makes a question follow work it may no longer be about, and requires the tool to decide that two tasks are the same work — which is exactly the kind of inference the rest of the design refuses to make.

Widening the gate to any unresolved question in the repository makes one team's blocker refuse another's unrelated completion, in a tool whose scope model exists precisely so that concurrent work does not interfere.

A third shape — reporting an unresolved question whose task is no longer active as drift — surfaces it without over-blocking, and is the likeliest answer. It is a decision, not a patch, so it is recorded here rather than guessed at.

It was found by running the documented end-to-end sequence as a demonstration, not by review, which is the argument for demonstrating a workflow end to end rather than testing its parts.
