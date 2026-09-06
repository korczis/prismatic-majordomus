+++
title = "handover writes an append-only record with computed front matter and required sections"
description = "majordomus handover takes a Markdown body on standard input and writes one new file under state/handovers/, named by timestamp, branch, short commit and a random suffix. The front matter — schema version, timestamps, task id, profile, owner, repository id, worktree, branch, head, working tree, changed files — is computed. The body must contain each section the policy requires (# Objective, # Current State, # Next Action by default), each non-empty; template placeholders in angle brackets count as empty. The file is created with mode 0600 through a temporary file and a hard link, so it cannot clobber an existing record; it lives under the ignored .ai/local/, so it is never staged or committed, and it reaches another checkout only by being carried there."
weight = 46
[extra]
claim_id = "handover-record"
status = "guaranteed"
source = "docs/claims/handover-record.md"
+++
{% raw %}

## What it means

`majordomus handover` takes a Markdown body on standard input and writes one new file under `state/handovers/`, named by timestamp, branch, short commit and a random suffix. The front matter — schema version, timestamps, task id, profile, owner, repository id, worktree, branch, head, working tree, changed files — is computed. The body must contain each section the policy requires (`# Objective`, `# Current State`, `# Next Action` by default), each non-empty; template placeholders in angle brackets count as empty. The file is created with mode 0600 through a temporary file and a hard link, so it cannot clobber an existing record; it lives under the ignored `.ai/local/`, so it is never staged or committed, and it reaches another checkout only by being carried there.

## How it works

`lib/handover.sh` validates the body with an awk section check, refuses identity fields in the body, writes the temporary file, links it to its final name with up to ten retries on collision, updates the task's checkpoint timestamp, and appends `task.handed_over` to the ledger. `--close` additionally sets the task's outcome to `handed_over` so a new task may start. `--resolve` finds the most relevant prior record for this worktree and branch — same worktree and branch first, then same branch, never a repository-wide fallback — and prints its git-state label and body.

## How to see it

```bash
printf '# Objective\nfix it\n# Current State\nhalf done\n# Next Action\nfinish it\n' | majordomus handover
# .ai/local/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md
printf '# Objective\nx\n# Current State\n\n' | majordomus handover
# majordomus: handover: missing or empty section(s): Current State, Next Action
```

## What it does not cover

Committing the record is a human decision; the command only prints its path. It does not summarise anything; the worker writes the sections.

## Why it exists

Append-only, atomic, never auto-staged, with required sections enforced at write time, was the shape of the one session-continuity protocol in the source environment that actually worked. The directory it replaced held hundreds of free-form notes with no schema.
{% endraw %}
