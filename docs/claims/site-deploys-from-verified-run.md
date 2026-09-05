# The site deploys from the same workflow run that validated it, after the jobs that guard the site are green, and nothing is rebuilt or measured twice for one commit

## What it means

A push to master produces one run. Its `site` job builds the site once, checks it, probes every route in a browser, and uploads those bytes as the Pages artifact; its `pages` job publishes them after `structure` (doctor, the derived data current), `suite` (every behavioural case), `rust` (the registry projections the Registry page reads are current) and `site` are green. There is no second workflow that repeats the suite and the probe over the same commit before deploying. Deployments queue in the `pages` job's own group; a pull request's runs are superseded by its next commit; master runs are independent of one another.

## How it works

`.github/workflows/validate.yml` is the only workflow. On master the `site` job keeps `site/public` as the artifact `site-public` after the build, the check and the probe; the `pages` job needs `plan`, `structure`, `suite`, `rust` and `site`, holds the `pages` concurrency group without cancellation, downloads that artifact and publishes it with `scripts/site-deploy --skip-build`, the same script a person deploys with. The workflow's own concurrency group is the pull request for `pull_request` and the run itself otherwise, cancelling in progress only for `pull_request`. `test/cases/26_ci_wiring.sh` proves there is one workflow, that it deploys, that the `pages` job needs exactly those jobs, and the concurrency rule; the shell doctrine `ci` in `lib/doctor.sh` still requires `bash test/run.sh` in it without a swallowed exit code.

## How to see it

```bash
ls .github/workflows/                                   # one file
awk '$0 == "  pages:" {f=1} f' .github/workflows/validate.yml | head -20
just test-shell 26_ci_wiring
gh run list --workflow validate --branch master --limit 3
```

## What it does not cover

Coverage, the macOS suite and the benchmark check gate merging through the `ci` status, not publishing: the old Pages workflow never waited for them either, and the deploy prerequisite is the same set of guarantees obtained from one run instead of two. Whether master is protected by the `ci` status is a repository setting on GitHub.

## Why it exists

The Pages workflow used to spend the better part of an hour per master commit repeating what the validation workflow had just done, and the deploy queue grew long enough that pushes superseded each other's deployments. Architecture wins over file continuity: the verified artifact of one run is what deploys.
