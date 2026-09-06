+++
title = "Re-explaining the same context to a brand-new session"
description = "Repository knowledge that only ever existed in a conversation has to be re-transmitted by hand to every worker that follows."
weight = 10
[extra]
id = "re-explaining-context"
status = "stable"
source = ".ai/repo/why/moments/re-explaining-context.md"
+++
{% raw %}

## The moment

A task ran long, the session ended, and the next one begins with the same paragraph you
typed yesterday: which files matter, what was decided, what is still open. Fifteen minutes
later the worker is where the previous one was — if you remembered everything.

## Why it happens

The conversation was the database. Everything the worker learned lived in a transcript that
the next session cannot read, so the only way to transfer it is to re-transmit it, by hand,
from memory. Long transcripts make it worse: the state is in there somewhere, diluted by
everything that did not matter.

The environments this tool was distilled from had a session-notes directory that grew to
ten gigabytes and a hand-written runbook for recovering working state after a session ended.
Notes had dates in their filenames that disagreed with their content, and nothing checked
either against git.

## Why a better model does not fix it

A model with a larger window still starts empty. The limit is not how much it can hold but
whether anything durable exists to hand it: a session that begins with no state has nothing
to load, however much it could load. Better models make the re-explanation faster to write
and no less necessary.

## What it costs

Fifteen to forty minutes at the start of every session, paid by the most expensive person on
the project, at the moment they have the least patience for it. The second cost is worse:
the re-transmission is from memory, so it is lossy, and the parts that get dropped are the
constraints nobody thought to mention — which is exactly how the next defect gets written.

## What Majordomus does

A task's durable facts live outside every conversation, in a handful of typed files. When a
session ends, `majordomus handover` writes an append-only record whose identity fields —
branch, head, working tree, changed files — are computed from git, never authored by the
worker. The body must carry `# Objective`, `# Current State` and `# Next Action`, each
non-empty, or the record is refused.

The next session runs `majordomus handover --resolve`. It picks the most relevant record for
this worktree and branch, never a repository-wide "newest note", and labels how far git has
moved since: `exact`, `advanced`, `diverged` or `different_context`. Stale state is named,
not silently trusted. `majordomus check --explain` prints the effective policy and profile so
the worker loads what the task needs and nothing more.

## Before and after

```text
before   session 2: "so, this repository is an umbrella of ..." (15 min, from memory)

after    session 1: majordomus handover     # Objective / Current State / Next Action
         session 2: majordomus handover --resolve
                    resolved t-20260903193012-a4f1  advanced (+3 commits)
```

## How to verify it

End a session with a handover and start another on the same branch. The record comes back
with a divergence label computed from git, not asserted; move the branch on and the label
changes without anyone editing the record.

## What it does not do

It does not summarise transcripts, and it refuses to store them. It does not decide what is
worth remembering; the worker writes the three required sections. It does not hook the
worker's runtime: the worker runs `handover` because its generated instructions tell it to.
{% endraw %}
