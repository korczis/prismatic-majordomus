+++
title = "Continuous integration"
description = "how a change is validated: the validation workflow over repository-owned gates, the planner and its model of what can affect what, the gates and how to run each locally, the caches and artifacts, the executable as a build output, the parallel suite and probe, the platform policy, and where the measurements live"
weight = 30
[extra]
source = "docs/CI.md"
+++

{% raw %}

How a change is validated: one workflow that is an adapter over repository-owned tooling, a
planner that reads one model of what can affect what, gates that run in parallel and only
when a change can reach them, and one status a branch rule can require. Publication is not
here — [`GITHUB_PAGES_PERFORMANCE.md`](@/docs/github-pages-performance.md) is the latency-sensitive
path that puts a master commit on GitHub Pages, and it runs beside this workflow rather than
after it. Every number about either lives in the measurements, never here:
`scripts/ci-baseline` records what GitHub observed into `.ai/repo/ci/baseline.json`, and
every run writes its own summary.

## The shape

```
plan ──► structure (always) ──┐
    ├──► suite                ├──► ci (the verdict; the one required status)
    ├──► rust                 │
    ├──► coverage             │
    ├──► bench (macOS)        │
    ├──► site                 │
    └──► macos ───────────────┘

and beside it, on the same commit, never after it:

pages.yml ──► one job: build ──► check ──► push gh-pages ──► measure publication
```

`plan` reads the model and decides. `structure` runs the cheap, deterministic gates every
plan has (`scripts/ci/shell-lint`, `scripts/ci/core-check`). The other jobs run when the
plan selected a gate they carry, in parallel, each through the script a person runs. `ci`
always runs and turns the plan and the jobs' results into one verdict.

Two workflows, and the split between them is what each decides. `.github/workflows/validate.yml`
decides whether a change may merge. `.github/workflows/pages.yml` decides what the public site
shows: it triggers directly on a master push whose paths can change the site and publishes as
soon as the bytes are proved, without waiting for gates that cannot change a published byte.
Neither repeats the other's work.

## The model: what can affect what

`.ai/repo/ci/gates.yaml` is the one place the dependency map lives. It declares the gates
(each with the job that carries it and the command it runs) and the path classes: a
pattern set and the gates a change under it can affect. `scripts/ci-plan` reads it and
nothing else; the workflow carries no path list of its own. The rules the planner applies
are written at the top of the model. In short: the always gates run in every plan; a
changed path selects the gates of every class it matches, as a union; a class may escalate
the whole plan; a path no class knows escalates it too, and the plan names the path; a
gate brings what it implies and what it requires.

```bash
scripts/ci-plan --check                       # the model resolves: every gate, job and reference
scripts/ci-plan --format text                 # what CI would run for the working tree against master
scripts/ci-plan --base origin/master --head HEAD
printf 'docs/DESIGN.md\n' | scripts/ci-plan --files - --format text
scripts/ci-plan --full "why" | jq .selected   # the full plan
```

Two execution classes, both evidence-driven:

- **affected** — a pull request: the gates of the classes its changed paths fall in. Nothing
  is skipped on a hope; a gate is left out only when no changed path is in a class that
  names it, and the plan says so for every gate.
- **full** — every gate: a push to master, the weekly schedule, a manual dispatch, a pull
  request labelled `ci:full`, a change to the pipeline itself (the workflow, the actions,
  `scripts/ci/`, the planner, the model, the runner), or a path the model does not know.

To force full validation of a pull request, add the label `ci:full`; the `labeled` event
re-plans it. To see why a gate ran or did not, read the `plan` job's summary or the
`ci-plan` artifact, or run the planner on the same diff locally.

`test/cases/94_ci_plan.sh` proves the relationships that matter: a Rust change selects the
Rust gates and never the site, a site change the site gates and never a Rust gate, a
document the site and the registry checks, the distribution every implementation, the
pipeline and an unknown path the full plan, an inert path only the always gates; and that
the model refuses a dangling reference. `test/cases/26_ci_wiring.sh` proves the workflow
is an adapter over the model: every gate's job exists, is gated on the plan's output for
that gate, is needed by `ci`, and `ci` always runs.

## The gates

The list, with the command each one runs, is the model: `scripts/ci-plan --full x --format
text` prints it, and `.ai/repo/ci/gates.yaml` explains each. The commands are the ones a
person runs:

```bash
scripts/ci/shell-lint                  # syntax and shellcheck over the tool, the scripts, the cases
scripts/ci/core-check                  # doctor, watch, context, continuity, plan validate, github-sync, site data
MJ_TEST_JOBS=4 bash test/run.sh        # the behavioural suite, four cases at a time
scripts/rust-check --ci                # every Rust gate but coverage, plus the benchmark check
scripts/rust-check --integration       # the executable built and the registry checks only
just coverage                          # line coverage against scripts/rust-coverage-threshold
scripts/site-build && scripts/site-check
SITE_PROBE_JOBS=4 scripts/site-probe   # every route at three widths, four routes at a time
```

`just ci-plan`, `just ci-structure`, `just ci-fast` and `just ci-full` are the recipes.

## The suite in parallel

`test/run.sh` runs the cases through a bounded pool when `MJ_TEST_JOBS` is set: that many
at a time, each with its own log, then the cases that declare `# majordomus-exclusive:
<reason>` one at a time, and the verdicts rendered in name order at the end with a failing
case's whole log before its line. Without `MJ_TEST_JOBS` it runs serially, streaming, as
it always has. The semantics are the serial runner's: a failing case turns the run red, a
filter that matches nothing is a usage error, an empty case directory is a usage error, and
`MJ_TEST_REPORT` writes one row per case (name, result, seconds, phase) for the summary.

The jobs that run the suite check out the whole history: a case that clones the checkout
into a fixture and pushes cannot push a shallow clone. A case may read the checkout it
lives in but must not write into it while other cases run;
the parallel phase checks `git status` before and after and fails naming the paths when
something changed. The cases that must write there (the two that build the site into
`site/public`, the one that edits and regenerates a derived document) carry the exclusive
header. The Rust cases drive the executable `MAJORDOMUS_BIN` names when it is set (CI
builds it once per job and hands it to every case), and build it once through cargo
otherwise; the two whose assertions are cargo's own (the crate's HTTP and projection suites,
the doc examples, the benchmark build) keep cargo.

## The browser probe

`scripts/site-probe` measures one route per browser process: the three widths are three
iframes of that route side by side in one page, each measured by the same script as
before (viewport against scroll width, the elements and text runs past the right edge, the
diagrams rendered against the diagrams declared). `SITE_PROBE_JOBS` bounds how many routes
are measured at a time; the findings are rendered in route order whatever the order the
processes finished in, and a finding still names the route, the width, the assertion and
what was measured. The homepage behaviour checks (the mobile menu, the dropdown, the theme
toggle) are unchanged. `--quick` remains the local sample.

## Caches and artifacts

Caches hold reusable dependency and build state; artifacts carry a job's outputs to a
later job or a later phase. Jobs run on isolated runners, so nothing relies on a shared
filesystem.

<div class="overflow-x-auto">

| cache | holds | key | invalidation | scope |
|---|---|---|---|---|
| npm (`actions/setup-node`) | the npm cache for `npm ci` | OS, `package-lock.json` hash | the lock file changes | restored from master when a branch has none |
| Rust (`Swatinem/rust-cache`, `.github/actions/setup-rust`) | the cargo registry, the git checkouts of dependencies, `target/` | job, OS, toolchain, `Cargo.lock` | the lock file or the toolchain changes; a red job saves nothing | per job, restored from master when a branch has none |

</div>


Nothing else is cached: Zola is one download, shellcheck one package, Chrome is on the
runner image. What a cache holds is never a source of truth: every gate reads the checkout.

<div class="overflow-x-auto">

| artifact | from | for | retention |
|---|---|---|---|
| `ci-plan` | `plan` | the verdict, a person asking why | short |
| `majordomus-cli-<target>` | `rust` | any later job or phase that needs the executable without building it: the debug binary and `majordomus-cli.json` (commit, target triple, rustc, `Cargo.lock` digest, profile) | short |
| `ci-metrics-<job>`, `ci-performance-metrics` | every job, `ci` | the timing rows of a run, gathered | longer |

</div>


## The Rust executable as a build output

`scripts/rust-check --artifact DIR` copies the debug executable into `DIR` with its
metadata beside it, and the `rust` job publishes that directory as
`majordomus-cli-<target>`. A consumer downloads it and sets `MAJORDOMUS_BIN`: the launcher
`bin/majordomus-mcp`, the suite and every Rust case honour that variable and never build
when it is set. Within one run the suite does not wait for the `rust` job: measured, the
suite job's own build from a warm cache costs seconds while waiting for the `rust` job
would serialise minutes, so the two run side by side and the artifact serves the phases
that come after a run (a release, a scenario runner over the real binary). Release builds
belong to a release workflow; every gate here uses the debug profile, which is what the
cases and the crate's own suites drive.

## Pages

Publication has its own workflow and its own document,
[`GITHUB_PAGES_PERFORMANCE.md`](@/docs/github-pages-performance.md). What matters here is the
boundary: this workflow's `site` job builds, checks and probes the site as a gate on merging,
and publishes nothing; `pages.yml` builds and checks the same commit again — cheaply, without
the probe and without a generation — and publishes. The duplication is deliberate and small:
the publication path cannot depend on a job it does not wait for, and what it repeats costs
seconds.

Concurrency: a pull request's runs here are superseded by its next commit; every other run of
this workflow is its own group and neither waits for nor cancels another, so a run held up by
a scarce runner does not hold up the next commit's evidence. Deployments are the opposite —
`pages.yml` cancels a superseded deployment, because the site is a projection of the newest
commit and finishing an older one would publish an older tree.

## Platforms

Linux is the blocking path for everything that does not depend on the platform: lints,
documentation, coverage, the benchmark runner. macOS runs what does: the behavioural suite
under the stock macOS shell (bash 3.2) and BSD userland, and the crate's own suites there
(files, signals, the lease, the spawned processes), when a change is in a class that can
reach them (the shell tool, the distribution, the crate, the pipeline) and in every full
plan. The benchmark check against a committed baseline runs on macOS, the one platform
with a baseline under `.ai/repo/benchmarks/rust/`. `docs/HARDCODING_LEDGER.yaml` records
the platform list as a deliberate decision.

## Telemetry and budgets

Every job writes a summary (`scripts/ci/summary`): the plan's selection, the cache hits,
the durations of its gates and steps (`MJ_CI_TIMINGS`, `scripts/ci/timed`), the slowest
cases of the suite (`MJ_TEST_REPORT`), the artifact it produced. The `ci` job gathers the
rows of every job into `ci-performance-metrics`. `scripts/ci-baseline` reads recent runs
back through the GitHub API and writes `.ai/repo/ci/baseline.json`: per run, per job and
per step, with the commit, the runner, the event and whether the caches hit, so a later
regression has a table to be compared with. The budgets are those measurements; a budget
written here would be stale the day after.

## Reproducing CI locally

```bash
scripts/ci-plan --format text                       # what would run for this working tree
just ci-structure                                   # the always gates
just ci-fast                                        # the gates the plan selects for this tree
just ci-full                                        # every gate
scripts/rust-check --ci                             # the Rust gate as CI runs it
just coverage
scripts/site-build && scripts/site-check && SITE_PROBE_JOBS=4 scripts/site-probe
MJ_TEST_JOBS=4 bash test/run.sh                     # the suite in parallel
bash test/run.sh                                    # serially
scripts/ci-baseline --runs 10                       # record what GitHub observed
```

`scripts/ci/verdict --plan plan.json --needs needs.json` renders the same verdict CI
renders, from a plan and a `needs` context; `test/cases/94_ci_plan.sh` drives it with
fixtures.
{% endraw %}
