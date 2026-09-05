# The behavioural suite runs a bounded number of cases at a time with the serial runner's semantics, a case that writes into the checkout declares itself exclusive and runs alone, and the parallel phase fails naming the paths when the checkout changed under it

## What it means

`MJ_TEST_JOBS=4 bash test/run.sh` runs the cases four at a time and means exactly what `bash test/run.sh` means: every case runs, in a disposable repository each, a failing case turns the run red with its own output in front of its verdict, the summary is one deterministic block in name order, a filter that matches nothing is a usage error, and so is an empty case directory. Each case runs in its own repository already, which is what makes them independent; the one thing they share is the checkout they read from, so a case that must write into it says so with `# majordomus-exclusive: <reason>` and runs after the pool, alone.

## How it works

The runner classifies the cases by their header, feeds the parallel ones to `xargs -P` (bounded; never a bare `&`), each worker writing the case's log, status and duration to files, then runs the exclusive ones one at a time, then renders the verdicts in name order from the files. Before and after the parallel phase it takes `git status` of the checkout and fails naming the paths that differ. `MJ_TEST_REPORT` writes one row per case (name, result, seconds, phase) for the job summary. Without `MJ_TEST_JOBS` the runner is the serial one it always was, streaming. `test/cases/94_ci_plan.sh` proves the semantics on a private harness of throwaway cases: the red run with the log rendered, the exclusive phase, the report, the bounded pool being faster than serial, the checkout guard naming the path, the usage errors; `test/cases/26_ci_wiring.sh` proves the failure semantics through both modes.

## How to see it

```bash
MJ_TEST_JOBS=4 MJ_TEST_REPORT=/tmp/suite.tsv bash test/run.sh
sort -t "$(printf '\t')" -k3,3nr /tmp/suite.tsv | head       # the slowest cases
bash test/run.sh                                            # serially, streaming
grep -l '^# majordomus-exclusive:' test/cases/*.sh          # the cases that run alone
just test-shell 94_ci_plan
```

## What it does not cover

The guard sees the checkout's tracked and untracked files, not ignored ones: a case writing under `site/public/` or `apps/majordomus-cli/target/` is not caught by it, and the two site cases declare themselves exclusive for that reason. A case that depends on the machine being idle (a wall-time assertion) is not made deterministic by the runner; case 65 was normalised for that. The worker count is a measured choice per runner, recorded in the CI model and the baseline, not a promise about any other machine.

## Why it exists

The suite ran one case after the other and took half an hour on a runner where its cases sum to a few minutes of work each; the long tail is wide and flat, so a bounded pool cuts the wall time without changing what is proved. The exclusive header keeps the invariant `project.tests-run-in-disposable-repos` states, cases never write into this checkout while others read it, explicit and checked instead of assumed.
