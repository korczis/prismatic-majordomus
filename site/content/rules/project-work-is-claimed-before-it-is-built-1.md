+++
title = "What already claims a path is checked before that path is built"
description = "What already claims a path is checked before that path is built"
weight = 135
[extra]
kind = "rule"
slug = "project-work-is-claimed-before-it-is-built-1"
identity = "project.work-is-claimed-before-it-is-built@1"
status = "active"
source = ".ai/repo/rules/project/work-is-claimed-before-it-is-built.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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

For a decision identity, neither of those is asked by hand any more, because asking by hand
is what failed: on 2026-09-11 three branches claimed ADR 0044 and two claimed 0043, and three
separate sessions each wrote the same survey to escape it. `majordomus adr next` is that
survey — every ref, every tag, every sibling worktree's working tree and the peer board, each
reporting what it claimed and saying so when it could not be reached — and it allocates above
the high-water mark rather than into a hole, because a hole is an identity that was taken and
withdrawn or taken and not yet written. Announce the number it gives you before you write it.

The other half of that is not advice. `majordomus adr check` refuses an identity this branch
adds that another ref already carries a different decision at, naming both documents and both
refs; `doctor` runs the same examination, and `doctor` is the pre-commit hook. That is what
makes this rule followable for identities: a claim that lived only in an unpushed branch used
to be unobservable until the branch was pushed, which is a precondition nobody could meet.
ADR 0053 is the reasoning; `test/cases/271_adr_allocation.sh` is the proof.

## Failure behaviour

`scripts/collision-check` exits 11 when a path is already claimed, naming the claiming
branches, their last commit and how long ago it landed; exit 0 means nothing else claims
the paths named; exit 2 is a usage error. The rule stays advisory, not blocking: a *path*
collision is sometimes the right thing to accept deliberately — two implementations worth
comparing, or a branch known to be abandoned — and nothing here can decide that for every
case mechanically. Promoting the whole rule because its identifier half became decidable
would make the tool refuse a judgement that belongs to a person.

The identifier half is gated even so, because there the judgement does not exist: two
decisions at one number is never deliberate. `majordomus adr check` exits 10 naming both
documents, both refs and `majordomus adr next`; `doctor` reports it as an `adr` failure and
refuses the commit.

## Verification

`scripts/collision-check` itself is the check for paths; running it is the only verification
that half has. Nothing forces a person or a session to run it before implementing, which is
why the provider bootstrap templates name it directly, next to the worktree paragraph, rather
than leaving it to be found.

For decision identities there is a verdict rather than a habit: `test/cases/271_adr_allocation.sh`
proves both directions — a tree that renumbers a decision onto an identity another ref already
carries is refused by `adr check` and by `doctor`, and a tree that takes what `adr next` says
is free passes — and the `adr-collision` gate in `.ai/repo/ci/gates.yaml` runs the same command
in CI.
{% endraw %}
