# Publication is gated on everything that can make the published site wrong and waits for nothing else; the checks that decide whether a change may merge run beside it on the same commit, not in front of it

## What it means

A push to master that can change the site starts one job, and that job publishes as soon as the bytes are proved: the committed derived data is current for the tree, the site builds from it, and every static check over the output passes. It does not wait for the behavioural suite, the crate's gates, coverage, the macOS suite, the benchmark check or the browser probe. Those run on the same commit, in parallel, in the validation workflow, and they gate merging through the `ci` status — they guard the repository, not the published bytes. Nothing that can make the published bytes wrong was moved off the publication path; what was removed from it is what could only ever have delayed it. A newer push supersedes an older deployment: an older run is cancelled rather than finished, because the site is a projection of the newest commit and the branch push is last-writer-wins. Publication is measured from outside rather than assumed from a green job: the site serves the commit it was built from, and the workflow waits for the public URL to name this one.

## How it works

Two workflows. `.github/workflows/validate.yml` plans from `.ai/repo/ci/gates.yaml` which gates a change can affect and runs them; `scripts/ci/verdict` turns that into the one status a branch rule requires. `.github/workflows/pages.yml` is the publication path: it triggers directly on a push to master whose paths can change the site — the `paths:` block is `scripts/pages paths`, the union of the gate model's classes that name the `site-build` gate plus the publication path itself — in a single job, holding `pages-<ref>` with `cancel-in-progress: true`, with `contents: write` and no other permission. The job runs `scripts/pages build`, `scripts/pages check` and then `scripts/site-deploy --skip-build`, the same script a person deploys with. `pages build` establishes that `site/data/generated` is current by comparing the tree's canonical input hash with the one the committed `source.json` carries — `generate-site-data --fingerprint`, one hashing process over the generator's own input list — and then renders it without regenerating it; a tree whose committed derived data is stale is refused, not silently regenerated, because a projection that disagrees with its sources is a defect to fix at the source. The model is `.ai/repo/ci/pages.yaml`: the trigger, the deploy path, the controlled budgets and the cache domains; neither the workflow nor the script carries a path list, a budget or a gate name of its own. Every phase is timed against those budgets in the job summary, with the Actions queue and GitHub's own "pages build and deployment" reported beside the controlled path as external latency rather than mixed into it. `test/cases/97_pages_fast_path.sh` holds the direct trigger, the derived paths, the cancellation, the minimal permissions, that no heavy gate is on the publication path and none has left the validation workflow, that only the publication measurement may fail softly, that the fingerprint is the value the generator writes, and that a build which skips the generation is byte for byte the site of a build that does not; `test/cases/26_ci_wiring.sh` holds the two workflows and the split between them, and the shell doctrine `ci` in `lib/doctor.sh` still requires `bash test/run.sh` in the validation workflow without a swallowed exit code.

## How to see it

```bash
scripts/pages paths                                     # what a push must touch to publish
scripts/pages budget                                    # the controlled budgets, from the model
scripts/pages benchmark -n 5                            # the controlled path, measured here
just test-shell 97_pages_fast_path
gh run list --workflow pages --branch master --limit 5
```

## What it does not cover

It does not make publication unconditional on correctness: master is what gets published, and what may reach master is decided by the `ci` status of the validation workflow, which is a repository setting on GitHub rather than a property of these files. It does not shorten the Actions queue, runner allocation, or GitHub's own Pages build of the `gh-pages` branch; those are measured and reported, and the end-to-end target is stated against them rather than enforced. It does not replace the browser probe: the probe still runs on every commit that can change the site, and a route it fails is a defect to fix, not a deployment that was blocked.

## Why it exists

Publication used to wait, per master commit, for every gate that decides whether a change may merge — the behavioural suite, the crate's gates and the browser probe among them — and the site therefore went public tens of minutes after the push that changed it, when it went public at all. The gates were not the problem; their position was. A projection of the newest commit is worth publishing the moment the projection is proved, and the proof that the projection is right is a different and much smaller thing than the proof that the repository is right.
