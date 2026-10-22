---
schema: adr/v1
id: adr-0157
kind: adr
title: A build under its floor asks the reaper before it refuses, and a goal given by name may take build output that has not landed
status: proposed
date: 2026-10-13
tags:
  - operations
  - safety
  - worktrees
provenance:
  origin: authored
related:
  - adr:adr-0055
  - rule:project.accumulation-is-measured
  - rule:project.reclaim-only-what-you-own
  - rule:project.destructive-sweeps-fail-closed
  - test:test/cases/276_disk_is_bounded.sh
  - test:test/cases/131_backlog_hygiene.sh
---

# 157. A build under its floor asks the reaper, and a named goal may take unlanded build output

## Context

ADR 0055 gave the disk a reaper and a floor and left one thing undecided: whether anything
runs the reaper. Nothing did. `mj_rust_space_check` refused a build under the floor and
printed the command that would have made room, for a person to type.

On 2026-10-13 that was not enough, twice over. Five sessions built at once behind a landing
queue; the volume fell from 36GiB to 20GiB free in fourteen minutes and to 13GiB an hour
later, and builds were stopped by hand at the floor twice. Nobody typed the printed command.
And when the reaper was asked, it found 4.3GB reclaimable out of roughly 110GB of build
output, because the rest belonged to worktrees whose branches had not landed: parked behind
the queue, nobody in them.

The owner asked for the clean-up to be automatic. The same morning a reduction that removed
worktrees took two live ones (#1056, held), which is the standing argument for the boundary
ADR 0055 drew: only build output, and only by a predicate measured on the machine.

Two rules bind what may be taken. `project.accumulation-is-measured` (version 2, point 5)
gives the reaper the build output of a worktree that is merged, unattended, clean and
provably cargo's own. `project.reclaim-only-what-you-own` says disk pressure is reported and
not resolved by taking somebody else's tools: a parked worktree's executable is what its
owner's commit hook runs.

## Decision

### The bound asks before it refuses

A build that finds the disk under its floor runs `scripts/reap-orphans --targets --reclaim`,
measures again, starts when there is room and refuses as before when there is not. This is
the timer ADR 0055 did not decide, placed where the need is measured instead of on a clock:
no scheduler, no new quantity. What it asks for by default is exactly what the printed
command gave, so no rule moves. The reaper's table goes to the standard error of the build
that asked.

`MAJORDOMUS_RECLAIM_UNDER_FLOOR` is `landed` (default), `unattended` or `off`. A test case
never asks on the machine's behalf: `test/lib.sh` sets `off`.

### A goal given by name changes the order, not the boundary

`scripts/reap-orphans --reclaim --until-free <size>` reclaims nothing while that much is
free. Below it, merged and clean stop being conditions and become the order: landed and
clean first, then clean and not landed, then worktrees with uncommitted files, the largest
first within each, stopping at the first candidate it does not need. Liveness, ownership,
cargo's mark, the exclusion of the primary checkout and of the checkout it runs from, and
the refusal as a whole when liveness, `origin/master` or the free space cannot be read are
unchanged.

This goes beyond point 5 of `project.accumulation-is-measured`, and it is therefore never a
default. It is taken by whoever was told to take it: a person who runs it, or a machine
whose owner set `unattended`. A build acting for one session does not take an unlanded
worktree's build from another on its own judgement.

## Consequences

- A build under the floor can now start without anyone typing anything, when landed
  worktrees hold enough.
- When they do not, the refusal is unchanged and the remedy is still a decision: the owner
  sets `unattended`, or somebody runs `--until-free` and says so on the board.
- Accepting this ADR owes `project.accumulation-is-measured` a version 3 whose point 5 names
  the goal as the one case where merged and clean are an order. Until then the rule text and
  the default agree, and the opt-in is the proposal.
- `scripts/ci/backlog-check` still holds that the reaper removes nothing but a resolved
  build directory; the new path removes the same `"$target"` and no other.
- Not decided here: reclaiming the primary checkout's own build output (47GB on the day),
  which every other checkout borrows from, and the shared intermediates' bound.
