+++
title = "Re-explaining the same context to a brand-new session"
description = "Why every fresh session starts from zero, and how durable state and handovers replace the transcript."
weight = 1
[extra]
hook = "re-explained the same context to a brand-new session"
responsibilities = ["state", "handover"]
commands = ["handover", "check"]
claims = ["handover-record", "no-transcripts", "git-identity", "divergence-label"]
+++
{% raw %}
## The moment

A task ran long, the session ended, and the next one begins with the same paragraph you typed
yesterday: which files matter, what was decided, what is still open. Fifteen minutes later the
worker is where the previous one was — if you remembered everything.

## Why it happens

The conversation was the database. Everything the worker learned lived in a transcript that
the next session cannot read, so the only way to transfer it is to re-transmit it, by hand,
from memory. Long transcripts make it worse: the state is in there somewhere, diluted by
everything that did not matter.

The environments this tool was distilled from had a session-notes directory that grew to
ten gigabytes and a hand-written runbook for recovering working state after a session ended.
Notes had dates in their filenames that disagreed with their content, and nothing checked
either against git.

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

## What it does not do

It does not summarise transcripts, and it refuses to store them. It does not decide what is
worth remembering; the worker writes the three required sections. It does not hook the
worker's runtime: the worker runs `handover` because its generated instructions tell it to.

## Try it

```bash
printf '# Objective\n…\n# Current State\n…\n# Next Action\n…\n' | majordomus handover
# next session, same branch
majordomus handover --resolve
majordomus check --explain
```
{% endraw %}
