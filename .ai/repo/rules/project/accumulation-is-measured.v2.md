---
id: project.accumulation-is-measured
version: 2
kind: rule
title: Everything that accumulates is measured, bounded and reaped by a committed command
description: Open pull requests, linked worktrees, local branches, running servers and the disk their build output holds all grow on their own and none of them shrinks on its own; each is measured by one committed command, carries a threshold that makes growth loud, and is reduced by a command that is safe to run unattended — never by a name match against a process list, never by deleting a worktree whose work is not published, and never by a predicate whose missing input becomes a verdict.
statement: Before adding anything that accumulates, name the command that measures it, the threshold that makes it loud and the command that reduces it; never wait on a forge merge button this repository's merge policy cannot reach, never match a process by name to kill it, never remove a worktree or branch whose work is not pushed, and bound what a build may consume before it starts rather than diagnosing the machine after it is full.
status: active
class: blocking
depends_on: [project.worktree-topology@1, project.work-is-claimed-before-it-is-built@1, project.destructive-sweeps-fail-closed@1, project.reclaim-only-what-you-own@1, project.every-wait-is-bounded@1]
tags: [operations, git, worktree, safety, agents, coordination, ci, disk]

x-majordomus:
  tests: [test/cases/131_backlog_hygiene.sh, test/cases/276_disk_is_bounded.sh]
---

# What changed in version 2, and why

Version 1 named four quantities: open pull requests, linked worktrees, local branches and
running servers. On 2026-09-12 a fifth one ended every session on this machine at once. The
volume reached **zero bytes free**, and what broke first was not a build but the harness
writing its own task files — so each session read its own death as broken tooling rather
than as a full disk, and the one command that would have said otherwise (`df`) could not be
run either. ~145GB of it was `apps/majordomus-cli/target` in worktrees whose branches were
already merged into `origin/master`, one of them for hours.

So version 2 adds disk, and it adds the half that no quantity had before: a **bound on the
demand side**. The other four are reduced after the fact and that is enough for them,
because a backlog of pull requests degrades throughput rather than ending the machine. Disk
is not like that: by the time it is measurable as a problem, nothing that could measure it
still runs. ADR 0055 carries the reasoning and the measurements.

# Rationale

Five things grow in this repository without anyone deciding that they should: pull requests
that are open, worktrees that are linked, branches that are local, servers that are running,
and the build output those worktrees hold. None of them shrinks on its own, and none of them
is visible from the place a person looks. The result is not a slow decline — it is a cliff.
This repository reached it twice: sixteen open pull requests, fifteen of which merged
cleanly on any developer's machine; sixty-four linked worktrees; a hundred and eleven local
branches; fourteen abandoned servers, one of them over a day old, together holding thirteen
ports and ~150MB. And then 180GB of build directories on a 926GiB volume, with 145GB of it
belonging to work that had already landed. From the outside the first symptom was a single
sentence — nothing is being deployed — and the second was every agent on the machine dying
in the same minute. Every individual part of the system was working exactly as designed.

The cause is shared across all five, and it is not laziness. Each is a quantity that only a
command can see. `git worktree list` is not run by the thing that creates worktrees; `gh pr
list` is not run by the thing that opens pull requests; a server's parent dies without
telling anyone; and nothing in finishing a piece of work removes the build directory it
produced. So each accumulates below the threshold of attention until it crosses a threshold
of consequence, and the first report is from a person, late.

The pull-request case has a second cause worth naming separately, because it looks like
neglect and is not. This repository resolves its derived artifacts with a custom git merge
driver (`merge=derived` in `.gitattributes`, `scripts/merge-derived`). A custom driver is
an executable configured per clone; GitHub cannot run one. So GitHub performs a plain
three-way merge over files that every branch regenerates, finds conflicts in all of them,
and marks the pull request `CONFLICTING`. **The merge button is not slow or flaky here; it
is structurally unavailable, and it will never become available.** A contributor who waits
for it waits forever, and the queue that builds behind that wait is the queue above.

The disk case has a second cause too, and it is the reason nobody fixed it for a week of
sessions: **no worker was allowed to.** `project.reclaim-only-what-you-own` forbids a worker
from removing another worktree's build output — correctly, after a session made a
colleague's executable vanish mid-run twice in one day — and closes with "disk pressure is
reported, not resolved by taking somebody else's tools". Every session on the machine that
morning obeyed it and announced the measurement instead. The rule was right and the gap was
real: the party who may resolve it is a *committed command that classifies every candidate
before it acts*, and until version 2 there was no such command.

# Required behaviour

1. **Each accumulating quantity has one measuring command.** Worktrees and branches:
   `majordomus worktree` (`majordomus_worktrees`). Abandoned servers and the build output of
   landed worktrees: `scripts/reap-orphans`. Landable pull requests:
   `scripts/ci/backlog-check`. A quantity with no command is not managed; adding one without
   its command is the defect this rule names.
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
5. **Build output is reclaimed by predicate, and only build output.** The reaper removes
   `apps/majordomus-cli/target` and never a worktree, so that a false positive costs a
   rebuild and can never cost a byte of source. A build directory is reclaimed only when it
   is merged into `origin/master`, nobody is working in that worktree, `git status` is
   empty, and the directory is provably cargo's own and inside that worktree. Liveness is
   read from the machine — `ps` and every process's working directory — and never from the
   peer board: an announcement belongs to a connection, and a worker that reconnects keeps
   its work while losing its place on the board, which is exactly the window a reaper runs
   in. Merged-ness alone reclaims the worktree of any session that has not committed yet.
   The primary checkout is excluded by name rather than by predicate, because every other
   checkout borrows its executable.
6. **A sweep that cannot measure liveness reclaims nothing.** Per candidate, an input that
   could not be read excludes that candidate and is counted in the report
   (`project.destructive-sweeps-fail-closed`). For liveness the refusal is total: a reaper
   that cannot tell the living from the dead reclaims nothing at all rather than everything.
   An mtime is never the predicate — it is the one whose missing input becomes an extreme
   value, `find -newermt` matches nothing on macOS, and a status sweep that refreshes an
   index fakes liveness for every tree it touches.
7. **A build is bounded before it starts.** Every path that starts a build on a worker's
   behalf asks one shared check whether there is room, and refuses below the floor. The
   floor is derived from a measurement of what one build of this crate consumes, and the
   measurement is written beside the number. A bound whose own input cannot be measured says
   so and lets the build run — the opposite of a deleting predicate, and deliberately so:
   refusing every build on a machine whose `df` is unreadable stops all work to prevent a
   hypothetical.
8. **Growth is loud before it is a cliff.** `scripts/ci/backlog-check` reports the landable
   backlog and fails when it exceeds its threshold, so that "nothing is being deployed" is
   a gate's finding and not a person's.

# Enforcement

- `scripts/ci/backlog-check` — the gate: the reaper and the integrator exist and are
  executable, the reaper matches processes by predicate and deletes only build directories,
  it still holds all three conditions for reclaiming one and still refuses when liveness is
  unmeasurable, every path that starts a build asks the shared space check, the doctrine
  names the forge constraint, no document advertises the merge button, and (with a token)
  the landable backlog is within its threshold.
- `test/cases/131_backlog_hygiene.sh` — the behavioural case for the servers and the gate.
- `test/cases/276_disk_is_bounded.sh` — the behavioural case for the build output and the
  bound: what the reaper refuses, and that a build below the floor does not start.
- `scripts/reap-orphans` — the reaper, dry-run by default, with `--kill` and `--reclaim` as
  two separate consents.
