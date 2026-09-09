---
id: project.work-is-claimed-before-it-is-built
version: 1
kind: rule
title: What already claims a path is checked before that path is built
description: Before implementing something that creates a new module, command group, document kind or generated artifact, a worker asks what already claims those paths across every local and remote branch (scripts/collision-check --new <paths>), not only who the peer board says is working now, and announces its own intent afterwards.
statement: Before creating a new module, command group, document kind or generated artifact, run `scripts/collision-check --new <paths>` against every branch, not only `majordomus_announce`'s peer board, and announce the intent once the paths are clear.
status: active
class: advisory
depends_on: []
tags: [coordination, process]
---

# Rationale

On 2026-09-09 eight sessions worked in this repository and two of them independently built
a release subsystem — the same module path, the same command group, the same generated
changelog — for several hours. Both had called `majordomus_announce`. Neither announcement
existed when the other started. A peer board is a process's memory: it says who is working
now, and it says nothing about a branch pushed an hour before this process started. The
durable signal was there the whole time and nothing looked at it — `origin/int/land-all`
already carried `apps/majordomus-cli/src/release/` before either session began. Git already
knew the answer; the protocol just never asked it.

Two designs arriving at one path is not a merge conflict, resolved by combining two diffs.
It is a choice between two finished pieces of work, and the one that loses was written for
nothing — hours of one session's time, spent on a question a `git` query would have
answered before it started.

# Required behaviour

Before implementing anything that creates a new module, command group, document kind or
generated artifact, ask what already claims those paths: `scripts/collision-check --new
<paths>` reads every local and remote branch for a version of those paths that differs from
the base, not only the branches active right now. Announce afterwards, once the paths are
clear or once a collision has been read and understood — `majordomus_announce` says who is
working now, `collision-check` says what was already built.

The check reads *committed* refs, local and remote. Work that is staged or uncommitted is
invisible to it, and so is an identifier a session is about to allocate but has not pushed —
two sessions renumbering an ADR into each other in the same minute is exactly what a branch
scan cannot see. So the two halves are not interchangeable: `scripts/collision-check` answers
what has been pushed, `majordomus_announce` answers what is being written right now, and the
intent announced should name the identifiers being allocated as well as the paths.

# Failure behaviour

`scripts/collision-check` exits 11 when a path is already claimed, naming the claiming
branches, their last commit and how long ago it landed; exit 0 means nothing else claims
the paths named; exit 2 is a usage error. The rule is advisory, not blocking: a collision is
sometimes the right thing to accept deliberately — two implementations worth comparing, or
a branch known to be abandoned — and nothing here can decide that for every case
mechanically.

# Verification

`scripts/collision-check` itself is the check; running it is the only verification this
rule has. Nothing forces a person or a session to run it before implementing, which is why
the provider bootstrap templates name it directly, next to the worktree paragraph, rather
than leaving it to be found.
