---
id: project.a-derive-takes-the-machine-lock
version: 1
kind: rule
title: A derive takes the machine lock itself, and no session's agreement is needed for it to hold
description: Every run of scripts/derive takes the one lock beside the primary checkout before its first stage and releases it on every exit; a stale lock is reclaimed only when its recorded holder is gone, a priority marker lets through only the branches it names, and an interrupted derive stops every process it started.
statement: scripts/derive takes <home>/.derive.lock through lib/machine_lock.sh before it builds or generates anything, waits while a live process holds it or a priority marker names another branch, reclaims it only from a recorded pid that no longer runs, releases only its own on exit, and stops its descendants on an interrupt; no caller is asked to take the lock for it.
status: active
class: blocking
depends_on: [project.derived-once@1, project.no-claim-without-test@1]
tags: [derive, machine, concurrency, sessions, enforcement]

x-majordomus:
  tests: [test/cases/986_a_derive_takes_the_machine_lock.sh]
---

# Rationale

A derive rewrites every derived artifact of a checkout and takes most of the machine while it
runs. Every worktree of this repository lives on one machine, and on 2026-10-09 eleven
sessions worked in it at once. They took turns through a directory `.derive.lock` beside the
primary checkout, by agreement, and the agreement failed in every way an agreement can in one
day:

- a session ran `just derive` without the lock beside one that held it;
- a session restarted and left the lock under its name with nothing running, so every other
  derive waited for nobody;
- waiting loops were killed with the session that started them;
- a stopped derive's site generator ran on for an hour, reparented to init, beside the next
  derive;
- a priority marker, left so that the integration queue's next pull request derived first,
  was read by some sessions and not by others.

The integration queue waits on derives: a pull request behind master is merged up and derived
before it can land. Every failure above held the queue. None of them was a careless person; the
protocol lived in sessions' memories and in a workflow document, and the program that did the
work knew nothing of it.

# Required behaviour

- `scripts/derive` sources `lib/machine_lock.sh` and calls `mj_lock_acquire` before it builds
  the executable or runs any stage. No caller takes the lock for it.
- The lock home is the directory that holds the repository's primary checkout, found from git's
  common directory, so every worktree agrees on it without configuration.
- The owner line names who, `pid=`, `branch=`, the worktree and the time.
- While a live process holds the lock, or `.derive.priority` exists and does not name this
  branch, the derive waits, says so when the wait starts and once a minute, and names the
  holder or the marker.
- A lock whose owner line names a pid that no longer runs is reclaimed, and the reclaim prints
  the line it replaced. A lock whose owner names no pid — taken by hand — is never reclaimed.
- A lock held by an ancestor of the derive is its own: the derive does not wait for its caller
  and does not release what its caller took. `MJ_DERIVE_LOCK_HELD` is how a caller that took
  it by hand, the old way, says so.
- The lock is released on every exit, and only by the process whose pid the owner line names.
- An interrupt (INT, TERM, HUP) stops every descendant of the derive before the lock goes.

# Failure behaviour

`test/cases/986_a_derive_takes_the_machine_lock.sh` drives the library over a lock home of its
own and fails when a free lock is not taken or its owner line does not name the taker; when a
stale lock is not reclaimed or the reclaim is silent; when a live holder's lock is taken, or the
wait is silent; when a lock taken by hand is removed; when a marker for another branch is passed
or one naming this branch is not; when a derive waits for its own caller or releases its lock;
when a process releases a lock another pid owns; when a descendant survives an interrupt; and
when `scripts/derive` does not take the lock before its first stage, release it on exit, or stop
its tree on an interrupt.

What it does not catch, stated rather than implied: a derive run by a program other than
`scripts/derive` (a hand-typed `scripts/generate-site-data`) takes no lock; `scripts/derive-check`
takes none, because it runs in the commit hook and must not wait minutes for another checkout;
and a pid that the system has reused for an unrelated process keeps a stale lock alive, which is
the safe direction — the derive waits, and says who it waits for.

# Verification

```sh
bash test/run.sh 986_a_derive_takes_the_machine_lock
```
