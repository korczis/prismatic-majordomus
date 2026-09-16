---
id: project.a-worker-that-stops-leaves-its-work-behind
version: 2
kind: rule
title: A worker that stops leaves its work where somebody else can find it
description: Work that exists only in one worker's working tree is invisible to every other and dies with the worker; anyone who fans work out is responsible for checkpointing it, because a worker's death is not a rollback. Where every unit of work is held is measured — a holding is a work tree, a branch or a stash, and each carries a disposition — and work held on one disk only is a finding of the completion invariant rather than a thing somebody might notice.
statement: Work is committed and pushed at the last point it was coherent, not at the point it was finished; whoever fans work out checkpoints it before it is lost; and a task is not finished while any holding of this repository exists on one disk only.
status: active
class: blocking
depends_on: [project.work-is-claimed-before-it-is-built@1]
tags: [process, integration]

x-majordomus:
  tests: [test/cases/383_convergence_is_enforced.sh]
---

# What changed in version 2, and why

Version 1 was advisory, and said exactly why in its own Failure behaviour: *"nothing can see
the uncommitted half except somebody looking, which is exactly why the responsibility is
written down rather than gated."*

That premise was true when it was written and is no longer true. The worktree topology
already counts the uncommitted work of every registered work tree, and one
`git rev-list --branches --not --remotes` names every commit that has never left this disk.
What was missing was never a measurement — it was a **verdict**: one word over the whole
repository, in a place something already refuses on.

The measurement was worth having the day it became possible. On 2026-09-16 this repository
held nine branch tips whose commits reached no remote, the oldest four days old, and
sixty-four work trees carrying uncommitted files, while every gate was green. None of that
was reportable by any command, so none of it was anybody's finding.

So version 2 is blocking, and `convergence.report` is what decides it: every holding — a
work tree with uncommitted files, a branch with commits, a stash — carries a disposition
(`integrated`, `published`, `local_only`, `uncommitted`), and the last two mean the work
exists on one disk only. The completion invariant's `no-stale-topology` question reads that
verdict instead of naming a command a person might run.

What version 2 deliberately does **not** do is delete anything. This rule is about work
being findable, and the remedy for every finding it makes is a commit or a push, never a
removal. `project.accumulation-is-measured@2` owns reduction, and already says that a
worktree or a branch whose work is not pushed is never removed.

# Rationale

On 2026-09-10 eight workers were running at once when the account they shared reached its spend
limit. All eight stopped in the same second, mid-sentence. Five had already pushed a branch and
opened a pull request, and lost nothing. Three had hours of work that existed only as modified
files in a worktree: twenty-seven files of repaired cases, twenty-two files of workflow model
plus a decision record, and two engine fixes. None of it was committed. None of it was visible
to anybody but the worker that made it, and that worker no longer existed.

It was recovered because somebody went looking for it deliberately, one worktree at a time,
and committed each as a checkpoint. That recovery was luck dressed as diligence: nothing in the
arrangement made those trees findable, and nothing would have flagged their loss. A later
worker cleaning disk would have removed them as abandoned.

The distinction that matters: **a worker stopping is not a rollback.** A transaction that
aborts leaves the world as it found it, which is why aborting is safe. A worker that stops
leaves a working tree — real work, in an unknown state, owned by nobody. The half-done state is
the normal outcome of interruption, not the exceptional one, and interruption is normal:
limits, timeouts, crashes, a laptop closing.

This is the same fact the repository already states for branches that never reach a remote —
work on one disk cannot be integrated and is one disk away from being lost. What was missing is
that it applies inside a single afternoon, to a working tree, and that the person who fanned the
work out is the one holding the risk.

# Required behaviour

**Work is committed at the last point it was coherent**, not at the point it was finished. A
checkpoint commit that says plainly it is unreviewed is worth more than a clean tree that
exists only on one disk.

**A worker who fans work out is responsible for checkpointing it.** Long-running work is
checkpointed and pushed periodically rather than at the end, and when a worker stops
unexpectedly its worktree is inspected and preserved before anything else is done with it — in
particular before any disk is reclaimed.

**A checkpoint is labelled as one.** It says it is unreviewed and unverified, so that whoever
picks it up treats it as a draft to be judged rather than as work someone stood behind. Whoever
resumes it reads the diff and keeps, fixes or rejects each part, and replaces the checkpoint
with real commits before it is offered for review.

**An interrupted worker's tree is classified, never assumed.** A worktree whose branch is fully
merged may still hold authored files that were never committed; absence of unique commits is
not absence of work.

# Failure behaviour

`convergence.report` refuses the claim, not the work: a holding whose disposition is
`local_only` or `uncommitted` makes the repository unconverged, and the completion
invariant's `no-stale-topology` question fails with the holdings named and the command that
would move each one out of danger. Nothing is deleted, nothing is committed on a worker's
behalf, and no branch is touched — the finding is that work is somewhere only one disk can
see it, and the remedy is a push or a commit the worker makes.

A repository the verdict cannot be read in reports `unknown` rather than `pass`: a
measurement that could not be taken is not a clean measurement.

# Verification

- `convergence.report` — the verdict, on the command line (`majordomus convergence`), over
  HTTP, on MCP and in the Cockpit, from one reading of the worktree topology, the commits
  no remote reaches and the stash list.
- `test/cases/383_convergence_is_enforced.sh` — the behavioural case, against a fixture
  repository: a branch that reaches no remote and an untracked file are each reported as at
  risk with their remedy; pushing and committing converges the same repository; and the
  done invariant answers `no-stale-topology` from the verdict rather than from a sentence
  about a command.
- `doctor`'s `clone` finding still reports branches whose commits reach no remote; the
  verdict is the same fact for the whole repository, including the half — uncommitted files
  — that nothing could see before.
