+++
title = "A task will know which commits it produced, rather than inferring them from a range"
description = "**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered."
weight = 75
[extra]
claim_id = "task-commit-attribution"
status = "planned"
source = "docs/claims/task-commit-attribution.md"
+++
{% raw %}

## What it means

**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered.

Scope is a claim about which paths *this worker* touched. `check` answers it with `git diff <base>..HEAD` plus the working tree — and a commit range has no author. When another session commits to the same branch in the same checkout, every file that session touched is reported as outside your scope. The report is correct about the files and wrong about who changed them.

The intended shape is that a task knows the set of commits it produced, because it recorded each one as it happened, instead of inferring them from two endpoints.

## How it works today

`mj_git_touched` in `lib/common.sh` unions `git status --porcelain` with `git diff --name-only <recorded head> HEAD`. `check`, `finish` and `watch` all measure scope against that set. It is exactly right for one worker on one branch, and it degrades the moment that assumption breaks.

The `worktree` field on the task record fixed the neighbouring problem — a record from *another checkout* no longer enforces its scope on yours. This one is different: the checkout genuinely is the same, so no identity field can separate the two workers.

## How to see it

```bash
majordomus start "my task" --scope lib/auth
# another session commits to this branch, in this checkout, touching docs/ and site/
majordomus check
# FAIL scope  docs/README.md — outside claimed scope (lib/auth)
# FAIL scope  site/templates/base.html — outside claimed scope (lib/auth)
```

Neither file is yours. The remedy today is `majordomus handover --close` and starting again at the current commit, which is what `docs/CONTINUITY.md` documents under "What this does not do".

## What it does not cover

Two properties are deliberate refusals rather than open questions, and they are what make the shape worth building rather than merely convenient.

**The list is a lookup, never authority.** Every answer it gives is verified against the commits themselves, and where the two disagree git wins and the disagreement is the finding. That is the same rule the task record already follows for `head` and `branch`; it is not a new principle, only an existing one applied to a new field.

**A rewritten commit is reported as unrecognised, not silently followed.** An amend or a rebase means the recorded commit no longer exists. Quietly resolving that to "probably this one" would make the record softer than git, which defeats the reason for recording it at all.

Beyond those: it will not attribute work inside a single commit, it will not know about commits made with `--no-verify`, and it says nothing about who authored a line.

## Why it exists

It was found by using the tool. Four task records were closed and restarted in one evening because peers committing to the same branch made each task's base go stale — the scope check was right every time and useless every time.

The observation that resolved the design was that the missing fact is about the commit, not about the task, and that git already carries it: every commit has an author and a time, and the `pre-commit` hook that already runs `doctor` is standing at the one moment when the answer is free. That framing also makes a second question cheap — `history` could then answer "what did this task change" without a diff at all — and a design that makes an unrelated question easier is usually cutting along the grain rather than against it.

It is bigger than the `worktree` field, it needs an answer for amend and rebase, and it is recorded here as the direction rather than left as folklore in a message thread.
{% endraw %}
