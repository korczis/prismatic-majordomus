+++
title = "A worker that stops leaves its work where somebody else can find it"
description = "A worker that stops leaves its work where somebody else can find it"
weight = 60
[extra]
kind = "rule"
slug = "project-a-worker-that-stops-leaves-its-work-behind-1"
identity = "project.a-worker-that-stops-leaves-its-work-behind@1"
status = "active"
source = ".ai/repo/rules/project/a-worker-that-stops-leaves-its-work-behind.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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

## Failure behaviour

Decided by review. `doctor` already reports branches whose commits reach no remote, which
catches the committed-but-unpushed half; nothing can see the uncommitted half except somebody
looking, which is exactly why the responsibility is written down rather than gated.

## Verification

Held by review, and by the `clone` finding `doctor` reports for unpushed commits.
{% endraw %}
