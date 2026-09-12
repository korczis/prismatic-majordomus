+++
title = "Everything that accumulates is measured, bounded and reaped by a committed command"
description = "Everything that accumulates is measured, bounded and reaped by a committed command"
weight = 60
[extra]
kind = "rule"
slug = "project-accumulation-is-measured-1"
identity = "project.accumulation-is-measured@1"
status = "active"
source = ".ai/repo/rules/project/accumulation-is-measured.v1.md"
+++
{% raw %}

## Rationale

Four things grow in this repository without anyone deciding that they should: pull requests
that are open, worktrees that are linked, branches that are local, and servers that are
running. None of them shrinks on its own, and none of them is visible from the place a
person looks. The result is not a slow decline — it is a cliff. This repository reached it:
sixteen open pull requests, fifteen of which merged cleanly on any developer's machine;
sixty-four linked worktrees; a hundred and eleven local branches; and fourteen abandoned
servers, one of them over a day old, together holding thirteen ports and ~150MB. From the
outside the symptom was a single sentence — nothing is being deployed — and every
individual part of the system was working exactly as designed.

The cause is shared across all four, and it is not laziness. Each is a quantity that only a
command can see. `git worktree list` is not run by the thing that creates worktrees; `gh pr
list` is not run by the thing that opens pull requests; a server's parent dies without
telling anyone. So each accumulates below the threshold of attention until it crosses a
threshold of consequence, and the first report is from a person, late.

The pull-request case has a second cause worth naming separately, because it looks like
neglect and is not. This repository resolves its derived artifacts with a custom git merge
driver (`merge=derived` in `.gitattributes`, `scripts/merge-derived`). A custom driver is
an executable configured per clone; GitHub cannot run one. So GitHub performs a plain
three-way merge over files that every branch regenerates, finds conflicts in all of them,
and marks the pull request `CONFLICTING`. **The merge button is not slow or flaky here; it
is structurally unavailable, and it will never become available.** A contributor who waits
for it waits forever, and the queue that builds behind that wait is the queue above.

## Required behaviour

1. **Each accumulating quantity has one measuring command.** Worktrees and branches:
   `majordomus worktree` (`majordomus_worktrees`). Abandoned servers:
   `scripts/reap-orphans`. Landable pull requests: `scripts/ci/backlog-check`. A quantity
   with no command is not managed; adding one without its command is the defect this rule
   names.
2. **Landing is local, and the forge is never waited on.** A pull request is merged on a
   machine that has the `derived` driver configured (`just derive-merge-driver`), on an
   integration branch, with the derived artifacts regenerated after the merge and before
   the push. GitHub's `CONFLICTING`/`DIRTY` is not a false positive — the conflict is real for
   any clone without the driver, and GitHub is always one. The answer that decides whether
   the work can land is the one from a clone that has it, taken from `merge-tree`'s **exit
   code** and never from a grep of its output. No document,
   workflow or instruction may tell a contributor to use the merge button.
3. **A process is reaped by predicate, never by name.** Sessions here are each other's
   instruments. A server is ended only when its parent is init, it has zero established
   connections, and it is older than a floor — each re-checked immediately before the
   signal. `pkill -f majordomus` is banned outright: it kills the servers live peers are
   attached to.
4. **Nothing unpublished is deleted.** A worktree or branch is removed only when its work is
   pushed. Zero unique commits is not proof: a merged branch can hold uncommitted and
   untracked authored work, and most worktrees here are dirty. Classify the working tree,
   not only the history.
5. **Growth is loud before it is a cliff.** `scripts/ci/backlog-check` reports the landable
   backlog and fails when it exceeds its threshold, so that "nothing is being deployed" is
   a gate's finding and not a person's.

## Enforcement

- `scripts/ci/backlog-check` — the gate: the reaper and the integrator exist and are
  executable, the doctrine names the forge constraint, no document advertises the merge
  button, and (with a token) the landable backlog is within its threshold.
- `test/cases/131_backlog_hygiene.sh` — the behavioural case.
- `scripts/reap-orphans` — the reaper, dry-run by default.
{% endraw %}
