# 6. CI planned from one model of what affects what, gates as repository-owned scripts, and the site deployed from the run that verified it

Status: accepted, 2026-09-05.

## Context

Validation ran as two workflows. `validate.yml` ran two operating-system matrices of every
gate on every push and again on every pull request of the same commit, and `pages.yml` ran
the behavioural suite and the browser probe a third time over every master commit before
deploying. The suite ran its cases one after the other, the probe launched a browser per
route and width, and the behavioural cases built the Rust crate cold inside the suite with
no cache. The measurements are recorded in `.ai/repo/ci/baseline.json`
(`scripts/ci-baseline --table`): a Pages deployment took the better part of an hour, queue
times reached tens of minutes from the duplication, and the macOS suite was red on master
from two constructs the stock macOS shell handles differently. Nothing was required, so
nothing was pending, and nothing was protecting master either.

## Decision

- **One workflow, an adapter.** `.github/workflows/validate.yml` orchestrates and
  implements nothing: every gate is a script a person runs (`scripts/ci/shell-lint`,
  `scripts/ci/core-check`, `test/run.sh`, `scripts/rust-check`, `scripts/site-build`,
  `scripts/site-check`, `scripts/site-probe`), and the Rust job runs `scripts/rust-check`
  itself with the mode the plan chose rather than a second list of its steps. The
  repeated setup is two composite actions (`.github/actions/setup-rust`,
  `.github/actions/setup-site`). `pages.yml` is gone; its guarantee is the `pages` job of
  the same run, which deploys the bytes the `site` job built, checked and probed.
- **One model, one planner.** `.ai/repo/ci/gates.yaml` declares the gates and the path
  classes with the gates each can affect; `scripts/ci-plan` computes the plan from it and
  the changed paths; the workflow's jobs are gated on the plan's outputs and carry no path
  list. A change to the pipeline or a path the model does not know runs everything. The
  model is data beside the benchmark policy, tested by `test/cases/94_ci_plan.sh`, and the
  workflow's agreement with it by `test/cases/26_ci_wiring.sh`.
- **One required status.** `scripts/ci/verdict`, in a job that always runs and needs every
  other, is green only when planning succeeded and every selected gate's job succeeded; an
  empty selection is red. Path-filtered workflows, which leave a required check pending
  when they never start, are refused.
- **Bounded parallelism, unchanged semantics.** `test/run.sh` runs `MJ_TEST_JOBS` cases at
  a time through `xargs -P`, renders verdicts in name order from per-case logs, runs the
  cases that write into the checkout (declared by `# majordomus-exclusive:`) alone, and
  fails when the checkout changed under the pool. `scripts/site-probe` measures one route
  per browser process with the three widths as iframes, `SITE_PROBE_JOBS` at a time, and
  renders findings in route order. Neither changes an assertion.
- **The executable as a build output.** `scripts/rust-check --artifact DIR` publishes the
  debug executable with its provenance; every Rust case and the launcher honour
  `MAJORDOMUS_BIN`. Within a run the suite builds from the warm cache rather than waiting
  for the Rust job, because the wait was measured to cost more than the build.
- **Triggers and concurrency.** Pull requests, pushes to master, a weekly schedule and a
  manual dispatch; no push trigger on other branches, since the same commit is validated as
  its pull request. A pull request's runs are superseded by its next commit; master runs
  never cancel, and deployments queue in order.
- **Platforms.** Linux is the blocking path for what is platform-independent. macOS runs
  the behavioural suite under the stock shell and the crate's suites, when a change can
  reach them and in every full plan, and the benchmark check where the committed baseline
  is. The bash 3.2 defects that kept the macOS suite red were fixed in the same change.

## Alternatives rejected

- Path filters on workflows (`on.push.paths`): the cheapest way to skip work and the way a
  required check stays pending forever; and a second copy of the dependency map in YAML.
- A separate Pages workflow consuming the validation run's artifact through
  `workflow_run`: two workflows to keep in step and a second run to wait for, for nothing
  the `pages` job of one run does not give.
- Sharding the suite or the probe across several runners: more runners for a small gain
  once one runner runs the cases four at a time; measured in the baseline.
- Making the suite wait for the Rust artifact: the serialisation costs more than the
  warm-cache build it saves.
- Dropping macOS or the benchmark check for being red: a red gate is evidence, not noise
  to remove; the shell defects were fixed and the benchmark check kept where its baseline
  is.

## Consequences

The model must be kept true: a new consumer of a path is a class edit, reviewed with the
case that names representative paths. A gate is added by adding a script, a gate entry and
a job, and the wiring case refuses a gate without its job. The measurements, not this
record, say how fast CI is: `scripts/ci-baseline` records what GitHub observed, every job
writes its summary, and `.ai/repo/ci/baseline.json` is the file a later regression is
compared with. Branch protection remains a repository setting; the status to require is
`ci`.
