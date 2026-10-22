# Handing a pull request over

A pull request reaches the trunk one at a time, decided against the trunk as it stands at
that moment (ADR 0101, `project.integration-follows-the-current-master`). That decision is
made by `majordomus prs` and is documented in `docs/INTEGRATION.md`; nothing here restates
it. This document is the part before and after: what a session owes before it says a pull
request is ready, what it does once the work has landed, and what the sessions of one
machine take in turns.

It applies whenever more than one session works in this repository. The board
(`majordomus_peers`, `majordomus_announce`) says who is working where; read it first, and
remember that a short board is not an empty room.

## Before a head is reported

A pull request is small and says one thing. It is not composed into a batch with others:
each is integrated by itself, and a change that needs a version bump carries the bump
itself, after its author has told the integrator, because two such changes take their
numbers in order.

Its author reports a head — the branch and the commit — only when that head is green on its
own. On that head:

| what is true | shown by |
|---|---|
| the change is formatted, lint-clean and its tests pass | `cargo fmt --check`, `cargo clippy`, the cases it touches |
| every changed line of the crate is covered by a test that runs in the process | `scripts/ci/coverage-differential`; a test that only drives the built executable is not counted |
| the structure gates of the plan pass | the `runs:` lines of `.ai/repo/ci/gates.yaml` for the job `structure` |
| new files were staged before the derive and again after it | `git status`, by status and not by a guessed path |
| the derived data agrees with the tree | `scripts/derive`, then `scripts/derive-check` |
| the site builds, when the change touches what the site renders | `scripts/site-build` |
| no baseline grew, and a line the change made stale is gone | the `*-baseline.txt` files under `.ai/repo/` |
| the documents and claims say what the change made true | `scripts/ci/claim-proof-check --strict`, `scripts/ci/doc-command-check` |

With the head it says what is known to be red and why. A branch that is green where its
author looked and red where nobody ran a gate is not a head.

A red job on a pull request is its author's. It is fixed on that branch and the new commit
is reported; nobody else pushes to it. A CI run is left to finish before anyone reacts, so
that one round names every failure.

The exception is a run that cannot end well. A session that knows its pushed head will fail —
the trunk has moved under a check every branch must carry, or a newer head is about to
replace it — cancels that head's run itself, after reading from it whatever it was pushed to
find out. A doomed run holds the runners a real one is waiting for.

## Integration is the tool's

The order of the queue, whether the trunk may be brought into a branch, and which pull
request merges next are answered by `majordomus prs` from a fresh observation, and
recomputed after every merge. One session at a time holds the lease and drains; it uses a
worktree's executable and never one built in the primary checkout, because a rebuilt
executable there replaces the one the shared server was started from.

No session builds a batch by hand to get ahead of that queue. A hand-built batch lands
several changes under one verdict, and a regression in it belongs to nobody.

## After a landing

**The release.** After a merge, in this order: the trunk's own validation run; the release
verdict for that commit; the audit against the previous tag; the tag; the release record's
pull request; the smoke run; the published site verified at that commit. A release says
which gate stood for each thing it claims and names what was not verified
(`project.release-is-a-projection`). Before every release the documents are true, the claims
are proved, and the repository's accepted debt is lower than at the previous release.

**The plan.** Work that landed is closed in the plan by a session that reads its acceptance
criteria, not its title: the record's own validation is run, the evidence is recorded with
the command and what it showed, and the issue is verified and done ([`plan.md`](plan.md)).
Evidence a session did not produce itself names where it came from. An issue whose criteria
are partly met stays open with its `current_state` measured again.

**The projection.** Closing an issue changes what GitHub should show, and until GitHub
agrees every other branch's structure job reports the difference. So the projection is
applied once, by the integrating session's word, for that pull request, when its other
checks are green (`project.github-projection-gated`).

## One machine

Sessions on one machine share its processors and its disk, and two things they do cannot
overlap.

**One derive at a time.** `scripts/derive` rewrites the derived data of a checkout and takes
most of the machine while it runs. Sessions take turns through a lock in the directory that
holds the checkouts: a directory `.derive.lock`, made with `mkdir`, holding a file `owner`
with one line naming the session, the branch and the time.

- The lock covers the derive and nothing else. It is released on the line after the derive,
  not at the end of a longer command.
- A session removes the lock only while the owner line is still its own.
- Nobody breaks another session's lock. A lock whose holder is gone is reported, not removed.
- A session about to bring the trunk into a pull request for the queue may leave a file
  `.derive.priority` beside the lock, one line naming what it is for. A session that finds
  it waits and tries again; whoever left it removes it when that derive ends.

**One build directory per worktree.** Two worktrees sharing a cargo target rebuild each
other's work.

The disk is everybody's. A session deletes its own build output when a branch has landed and
its coverage verdict is in hand, and says how much it freed.

## What holds this, and what does not

| part | held by |
|---|---|
| one pull request at a time, against the current trunk, never around the branch protection | `majordomus prs` and its lease (`project.integration-follows-the-current-master`) |
| a feature branch is committed only from its own worktree | the pre-commit hook (`project.worktree-topology`) |
| derived data agrees with the tree | `scripts/derive-check`, in the commit hook and in CI |
| new code is covered | the coverage gate (`project.new-code-is-covered`) |
| the projection agrees with the plan | the `github-projection` gate |
| the derive lock, the priority file, a head reported by message, the plan closed after a landing | agreement only; nothing refuses a session that ignores them |

The last row is the honest one. The lock is a convention between sessions that chose to keep
it, and it has been held too long by a session that chained a second step after its derive.
Until a capability takes and releases it, the rule is the sentence above: the lock covers the
derive and nothing else.
