---
id: project.one-derive-at-a-time
version: 1
kind: rule
title: One derive at a time per machine, and a stopped derive leaves no orphans
description: scripts/derive holds the machine's derive lock for its whole run, yields to a priority marker that names another branch, reclaims a dead owner's lock aloud, and takes its whole process tree down when it is stopped.
statement: Run every derive, and every full suite, through scripts/derive (`scripts/derive --locked -- <command>` for a suite), never around it. The script takes the machine's lock, waits for a live owner and for a priority marker that names another branch, and on TERM, INT or HUP stops every process it started.
status: active
class: blocking
depends_on: []
tags: [derived, machine, coordination]

x-majordomus:
  tests: [test/cases/1018_one_derive_at_a_time_and_no_orphans.sh]
---

# Rationale

A derive rewrites every committed artifact. Under load it costs from minutes to an hour of
one machine shared by every session working this repository. On 2026-10-09 nine sessions
shared it at load 57. They serialised their derives through a lock directory and a priority
marker beside the primary checkout, but the convention lived only in the wrappers they wrote
around `scripts/derive`. One `just derive` without the wrapper ran beside a repair's locked
derive. Stopping it killed the shell and not stage B: bash defers a trap until a foreground
child returns. So `generate-site-data` ran on for an hour, reparented to pid 1, with nobody's
name on it. The integrator first blamed the wrong session for the lock it saw held.

A convention every worker has to remember is a convention one worker forgets. The script
that does the expensive work is the one place every derive passes through, so the lock and
the clean-up belong there. They live in `lib/machine_lock.sh`, which `scripts/derive`
sources, so the next script that needs the machine can source it too. d1 built the same lock
independently (`fix/a-derive-takes-the-machine-lock`) and retired it to review this one. Its
reading of owner lines without a pid is the one kept here.

# Required behaviour

- `scripts/derive` takes the lock before it builds anything. It is a directory made
  atomically by `mkdir`, with an owner file of `<tag> <pid>`. The default is
  `.derive.lock` beside the primary checkout, where the sessions kept it, derived from git's
  common directory and never written as a machine path. `MAJORDOMUS_DERIVE_LOCK` overrides it.
- The holder's pid is the owner line's `pid=<n>`, or its last word, and only when that is a
  number. A line with no numeric pid, such as a hand-taken lock or a loop that ended its line
  with a time, is a live holder that cannot be checked. It is never reclaimed and never taken
  for an ancestor. It is waited for, and the wait prints the line.
- It is re-entrant: when the owner's pid is an ancestor of the derive, the caller holds the
  lock and the derive runs under it, leaving it to the caller.
- A lock whose numeric owner pid is not running is reclaimed. The reclaim prints the dead
  owner's line. It is never silent.
- Only the process that wrote the owner line releases the lock, and only while the line is
  still exactly what it wrote.
- While `.derive.priority` beside the lock exists and does not name the derive's branch as a
  whole word, the lock is not taken. A marker naming `feature/x-two` does not name
  `feature/x`. `MAJORDOMUS_DERIVE_IGNORE_PRIORITY=1` is for the marker's own author,
  whose derive runs under another name.
- Every stage runs as a background child under `wait`, so a signal is handled at once. On
  TERM, INT or HUP the derive signals every descendant deepest first, kills the survivors,
  releases the lock and exits 143, 130 or 129.
- A derive that finds an orphan of an earlier derive of this checkout (parent pid 1, command
  under this checkout's `scripts/derive` or `scripts/generate-site-data`) stops it and its tree
  before it starts, naming it. This follows `project.reclaim-only-what-you-own`: only what
  this checkout's own derive started, matched by the absolute script path and the dead
  parent, never by a process name. `scripts/reap-orphans` keeps the same discipline for
  servers and build output.
- `scripts/derive --locked -- <command>` runs a command, such as a full suite, under the same
  lock and the same signal handling.

# Failure behaviour

A derive that waits says why, once per reason: who holds the lock, or what the priority
marker says. It gives up with exit 75 after `MAJORDOMUS_DERIVE_WAIT_MAX` seconds (default
14 400). A derive started around the script, by calling its stages directly, is outside this
rule's enforcement, and the rule's statement forbids it.

# Verification

`test/cases/1018_one_derive_at_a_time_and_no_orphans.sh` drives the lock through `--locked`
and proves, each by observation, these behaviours:

- taking and releasing the lock;
- reclaiming a dead owner's lock with the owner's name in the output, for both owner-line forms;
- never reclaiming an owner line with no numeric pid, including today's hand-written and
  time-stamped lines;
- release only by the process whose line it still is;
- waiting for a live owner without touching its lock, and giving up with exit 75;
- waiting for a marker that names another branch, or a longer branch with this one as a
  prefix, and running for one that names this one;
- re-entry under the old wrapper;
- a TERM that leaves no grandchild running and no lock behind;
- an orphan predicate, over a process table the case writes, that selects exactly this
  checkout's ppid-1 derives and never another worktree's, a prefix sibling's, a nested path's
  or a live derive's.

Each of these mutations fails the case: the tree kill removed, the reclaim notice removed,
the numeric-pid check removed, the whole-word marker match turned into a substring match, the
owner check on release removed, and the path's word boundary removed. The reaper's kill
itself has no live proof: a real ppid-1 process cannot be manufactured portably inside a
test, so its predicate is proved over the fixture instead.
