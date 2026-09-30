---
schema: adr/v1
id: adr-0063
kind: adr
title: A peer's uncommitted work is part of its claim, and a commit that carries it is refused
status: proposed
date: 2026-09-14
tags:
  - coordination
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0044
    - decision:adr-0053
---

# 63. A peer's uncommitted work is part of its claim, and a commit that carries it is refused

## Context

On 2026-09-14 three Claude sessions worked in one checkout of `catharsis-as-a-service`, on `main`. The
board was read and it worked as designed: `majordomus context` printed

> OVERLAP another worker claims ground this task claims: p3 claims
> `templates/components/illustrations.html`, inside your `templates` … a claim is not a lock; talk to them
> before you both write it

Half an hour later commit `ba9cb4c` — written by a session that had authored none of it — carried another
session's half-finished rename: four templates already using a renamed CSS class, and the stylesheet that
still declared the rule the rename removed. It reached `origin/master` and CI passed, because the
validator that would have caught the inconsistency was, at that moment, an uncommitted file belonging to
the session whose work had just been committed underneath it. The recovery was three cross-session
messages and a hand audit of which paths belonged to whom.

Nothing in the tool could have refused it. Both mechanisms that exist compare *declared strings*:
`check --overlap` reads other worktrees' `current.yaml` scopes and compares path containment, and the peer
board compares announced claims. Neither reads a working tree. And the decisive detail is that the author
of those edits — this session — had never called `majordomus_announce` at all: the paths that were
swallowed were claimed nowhere, by anyone, and were nonetheless obviously in flight to anything that
looked at `git status`. `scripts/collision-check` exists for the mirror-image case and says so in its own
header: *"a peer board is a process's memory — it says who is working now, and says nothing about the
branch somebody pushed an hour before"*. This is the third case: not a pushed branch, not an announcement,
but a dirty file on a disk two processes share.

## Decision

**A worker's claim includes the paths it has changed and not committed.** The board carries, per peer, the
path set of its checkout's uncommitted work — observed from `git status --porcelain`, refreshed when the
peer announces and when it checks. Observed, not declared: this is the half that cannot be forgotten,
because it is what the worker is actually doing.

**Observation does not require an announcement.** A session that never announced still contributes its
uncommitted paths to the board. The incident that forced this decision had no announcement on either side,
and a mechanism that only protects the polite is not a mechanism.

**A commit that carries a path another live peer has changed and not committed is refused, exit 15,
naming the peer and the paths.** This is the first refusal in this area — `check --overlap` exits 0 by
design and `majordomus.scope-integrity` refuses only paths outside your *own* scope. A refusal at the
commit is where the information is still cheap: after the push it costs a revert, a message and someone
else's afternoon.

**The escape is explicit, per path, and recorded.** `--carry <path>` commits a peer's in-flight file
deliberately — finishing someone's work is legitimate and sometimes necessary — and the ledger records
that it was carried, from whom. A flag that switches the whole check off is not offered.

**Liveness decides, and a dead peer claims nothing.** The board's own liveness is what makes a peer's
uncommitted work claimable; an entry for a session that is gone never refuses anything. Peers are
gathered repository-wide per ADR 0044, so the same checkout and a sibling worktree are the same question.

## Alternatives rejected

**Lock the files.** A lock outlives the process that took it, and this repository has already measured
what abandoned state costs it — 192 running servers, 64 worktrees, 47 of them dirty. A refusal needs no
cleanup: when the peer is gone, nothing refuses.

**Refuse only announced paths.** The incident had no announcement. Refusing on declarations rewards the
worker who declares and protects nobody from the one who forgets — and forgetting is the normal case, not
the deviant one.

**Warn harder.** The board already warned, in the exact words quoted above, and the commit still happened.
A warning that has been printed and ignored is not evidence that the warning was too quiet.

**Make each session take its own worktree.** It is the better arrangement and it does not close this: a
worktree is a choice a session makes at the start, and three sessions in one checkout is what actually
happened. The refusal costs nothing when the arrangement is already clean.

## Consequences

The board grows a per-peer path set, refreshed on announce and on check — a `git status` per refresh, on
the checkout the peer already has open.

`majordomus commit` gains a refusal and a per-path escape, and the ledger gains a record of every carry.

A worker can now be stopped by a peer's forgotten dirty file. That is the trade: the cost of a refusal at
the commit, against the cost of an inconsistent commit on a protected branch, which this repository has
now paid twice.
