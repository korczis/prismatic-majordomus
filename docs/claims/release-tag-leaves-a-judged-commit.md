# A release tag is pushed only from a commit whose ci check concluded success, and a push of release tags is judged by that verdict and never by the working tree it was made in

## What it means

A tag named `v` and a version is the request to publish: the release workflow starts from
it. This repository's pre-push hook does not let one leave the machine it was made on
unless the commit it names carries a `ci` check-run that concluded success. A commit whose
check failed, is still running, has not been created yet, or could not be read is each a
refusal, and git pushes none of the refs that came with the tag.

The working tree is not part of the question. A push that moves only release tags is not
asked `majordomus finish --check`, because a tag carries a commit and nothing a checkout
holds beside it. A branch in the same push, or a tag that is not a release tag, and the
finish contract judges the whole push exactly as it did.

A tag the remote already holds stays where it is while a release stands behind it: a push
that deletes or moves it is refused, and so is one made when the forge could not be asked.

## How it works

`.githooks/pre-push` reads the refs git hands it. For each one under `refs/tags/v[0-9]*` it
peels an annotated tag to its commit and runs `scripts/ci/release-verdict --commit` on it,
the command the release workflow's `plan` job runs first, so the question asked locally and
the one asked in the pipeline cannot drift apart. Anything but exit 0 refuses the push with
the tag, the commit and the verdict's own message; for a verdict that could not be read yet
the hook adds the remedy, which is to wait for master's validate on that commit and push
again.

`scripts/ci/release-verdict --wait N` is what makes waiting possible. The `ci` check-run is
created when the validate workflow reaches its last job, so a commit that is being judged
has none for most of the run. The wait polls an absent check-run exactly as it polls a
running one, until one concludes or N seconds have passed, and says at the bound which of
the two it was still looking at.

## How to see it

```bash
bash test/run.sh 1032_a_release_tag_is_pushed_only_from_a_judged_commit
bash test/run.sh 1033_release_verdict_waits_for_a_check_that_does_not_exist_yet
```

The first case drives the hook of the tree you are in against a scratch bare remote and a
scripted forge: a tag is pushed when its commit passed, refused with no check-run, a running
one, a failed one and an unreadable answer, and the remote is read afterwards to show the
tag is not there. It then dirties a file outside the task's scope, shows that the finish
contract refuses that tree, and pushes a tag from it; pushes a branch from it and is
refused; and tries to delete and to move a tag with a release behind it. The second case
answers "no check-runs" twice and then a success, and reads how often the verdict asked and
how long it slept.

## What it does not cover

The hook is this repository's own, installed by `core.hooksPath`; a clone that has not
wired its hooks, or a push made with `--no-verify`, is not asked. The release workflow asks
the same verdict again in `plan` for that reason, and it is that job, not the hook, which
keeps a red commit from being published.

The verdict is the newest `ci` check-run of the commit. It says the commit passed the gates
its plan selected, not that the commit is the one somebody meant to release: a tag on the
wrong green commit is pushed, and `docs/DISTRIBUTION.md` gives that mistake one answer, a
new version. Whether anyone has already pinned a tag that has no release behind it is not
something a hook can read, so such a tag may still be moved, and the hook says so when it
lets one go.

## Why it exists

One release failed three ways in two days. On 2026-10-09 the tag `v0.19.0` was pushed by
hand while validate was still running on its commit. The pipeline's `plan` job waited five
minutes for a `ci` check-run that did not exist yet, failed, and nothing was published
until somebody reran the workflow an hour later. Asking by hand had not helped either:
`release-verdict --wait 3600` answered in the first second that the commit had no check-run,
because the wait only ever watched a run it could already see.

The next day the tag `v0.19.1`, on a commit that had passed, was refused from the checkout
it was pushed from. The hook asked every push the finish contract, and that checkout held
another session's uncommitted file outside the active task's scope. So the one push whose
contents cannot include a working tree was stopped by a working tree, while the question a
release tag does raise had no one to ask it before the tag was public.
