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

x-majordomus:
  tests: [scripts/collision-check, scripts/ci/collision-gate, test/cases/325_the_collision_guard_has_a_caller.sh, test/cases/271_adr_allocation.sh]
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

# Failure behaviour

`scripts/collision-check` exits 11 when a path is claimed by a collision, naming the
claiming branches, their last commit and how long ago it landed; exit 0 means nothing
collides on the paths named; exit 12 means it could not measure — a shallow repository, a
base or head that does not resolve, or no branch besides the base to compare against — and
never reads as "nothing claims this"; exit 2 is a usage error. For a path the base does not
have, a branch carrying the same bytes (a rescue copy), sharing the commit that added the
path (work built on the other), or byte-identical on other paths the head adds (a copy of
the same work that has since diverged) is listed and is not a collision; only a path added
on both sides independently, with different content, is. A path declared `merge=derived` is
regenerated by the merge and is counted rather than examined.

The rule stays advisory as a habit — run the check before implementing — but it is no longer
only a habit. The `collision-check` gate (`scripts/ci/collision-gate`) asks it on every pull
request, over every path the pull request adds, and refuses a collision. A *path* collision
is sometimes the right thing to accept deliberately — two implementations worth comparing,
or an abandoned branch that is not yours to delete — and nothing here can decide that
mechanically, so the decision is left where it belongs: a person labels the pull request
`collision:accepted`, and the collision is still printed for the reviewer.

The identifier half is gated even so, because there the judgement does not exist: two
decisions at one number is never deliberate. `majordomus adr check` exits 10 naming both
documents, both refs and `majordomus adr next`; `doctor` reports it as an `adr` failure and
refuses the commit.

# Verification

`scripts/collision-check` is the check for paths, and until 2026-09-13 nothing ran it: the
provider bootstrap templates named it and no hook, recipe or workflow called it. The
`collision-check` gate in `.ai/repo/ci/gates.yaml` is its caller now, on every plan of the
structure job, which checks out at fetch-depth 0. `test/cases/325_the_collision_guard_has_a_caller.sh`
proves it against a fixture repository with a bare origin, in the shape CI has (a detached
merge commit): a new path another unmerged branch adds differently is refused and the branch
is named; a path only the pull request adds passes; the pull request's own branch, the pushed
branch it is built on and a same-bytes rescue copy are not collisions; a merged branch holding
an older version of a file is not a claimant; and a shallow or single-branch clone exits 12
rather than 0.

For decision identities there is a verdict rather than a habit: `test/cases/271_adr_allocation.sh`
proves both directions — a tree that renumbers a decision onto an identity another ref already
carries is refused by `adr check` and by `doctor`, and a tree that takes what `adr next` says
is free passes — and the `adr-collision` gate in `.ai/repo/ci/gates.yaml` runs the same command
in CI.
