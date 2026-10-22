---
id: project.a-derive-offload-waits-its-turn
version: 1
kind: rule
title: A derive offloaded to another machine goes through the declared hosts and waits its turn in their queue
description: majordomus offload derive is the one way a derive runs on another machine; the hosts and their limits are declared once under .ai/repo/offload, waiters are served in arrival order, a dead waiter blocks nobody, and each derive builds in a directory of its own.
statement: Offload a derive with `majordomus offload derive <worktree>`, never with a script of your own or by running scripts/derive on a host by hand. Declare a host in .ai/repo/offload/derive.yaml, never in a script; take the host's lock only through the queue.
status: active
class: blocking
depends_on: [project.one-derive-at-a-time@1]
tags: [derived, machine, coordination]

x-majordomus:
  tests: [apps/majordomus-cli/tests/offload.rs]
---

# Rationale

Every session needs a derive before it can commit. The shared MacBook takes up to an hour
for one under load, and an idle cluster machine about ten minutes. So the sessions offloaded
their derives with a script outside the repository, which carried the host addresses in its
source. Its lock on the host was `until mkdir lock; do sleep 15; done`. That is a race, not a
queue: on 2026-10-11 at 00:18Z five sessions waited on lundra, the oldest for 2 h 12 min,
while later arrivals took the lock ahead of it. At 00:25Z on the second MacBook, a derive run
by hand rebuilt the shared build directory under a running offloaded derive. Stage C refused
with "this executable is not of this tree's generation".

A convention in a home-directory script is one nobody reviews, tests or finds. The offload
is now a command of the executable, and its hosts are an object of the layer.

# Required behaviour

- The hosts, their ssh identity files, the free-space minimum, the queue's paths and the
  layout on a host are read from the `derive-offload` declaration
  (`.ai/repo/offload/*.yaml`, schema `derive-offload/v1`) and nowhere else.
- A host is chosen only when it answers and has at least its minimum free. Of those, the one
  with the fewest derives running or waiting is chosen, and a tie goes to the host declared
  first.
- Waiters on a host take its lock in arrival order. A ticket whose holder process is dead is
  removed by whoever sees it. A lock whose holders are all dead is reclaimed aloud, and a
  lock with no holder file to check is waited for.
- Each derive builds in a build directory of its own, cloned from the shared one and removed
  when it ends.
- What travels is the worktree as it stands, and a merge in progress travels with both
  parents. Untracked files and an unresolved merge are refused before anything travels. A
  derive that deletes a tracked file is refused before anything is applied. Nothing is
  committed or pushed.

# Failure behaviour

A refusal says what to do and exits with the code `docs/OFFLOAD.md` lists for it. A host that
cannot be asked is skipped with its reason. When none is left, the command refuses with 12
rather than deriving here. A derive run on a host by hand, or a lock taken by hand, is outside
this rule's enforcement, and the rule's statement forbids both.

# Verification

`apps/majordomus-cli/tests/offload.rs` proves these behaviours, with real holder processes
and hosts that are directories reached through a local stand-in for ssh:

- arrival order, although the later waiter started waiting first;
- a dead waiter's ticket that does not block;
- a dead lock reclaimed, and an uncheckable lock waited for;
- host choice that skips a host that is down or under its minimum;
- the staged, uncommitted round trip;
- a merge's second parent kept;
- each refusal;
- a concurrent build into the shared directory that does not replace the derive's executable.

Each of these mutations fails it: a lock taken regardless of position, dead tickets kept, and
the build directory pointed at the shared one.
