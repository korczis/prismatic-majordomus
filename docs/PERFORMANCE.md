# Performance truth

How this repository knows what is slow, why, and whether a change made it faster: a phase
report on every command, one benchmark harness derived from the command registry, local
evidence that is never a baseline, a tracked baseline that is never edited by hand, and a
check that refuses a regression by policy. The rules behind it are
`project.derived-once`, `project.benchmarkable-commands` and
`project.performance-evidence` under [`.ai/repo/rules/project/`](../.ai/repo/rules/project/).

## Where the time goes: `MJ_TIMING`

Set `MJ_TIMING=1` on any command and its phases and work counters are reported on stderr at
exit, ranked by time; stdout is untouched.

```
$ MJ_TIMING=1 majordomus doctor >/dev/null
TIMING clock=epochrealtime total=2365 ms
phase       648 ms     1 x  validate:context
phase       446 ms     1 x  ctxd:scan
phase       341 ms     1 x  validate:catalogue
...
count       108     yaml_flatten
count        22     git
```

A phase is declared where a command does one thing: the doctrine dispatcher declares one
per validator, the knowledge compiler one per stage, the site generator one per section.
A counter is one unit of work worth counting: a YAML flatten, a batch flatten, a batch
hash, a git call. The counters are the structural evidence: a loop that regressed to one
process per item shows up as a count that grows with the input, long before a clock notices
it on a small repository. The clock is `EPOCHREALTIME` where bash has it, perl where it
does not, whole seconds otherwise, and the report says which.

## What was slow, and the shape of the fix

Every hot spot found this way had the same shape: work that depends only on unchanged
canonical files, redone per item, each item paying a process. A process costs
milliseconds; a thousand of them cost the tool its users. The fixes are the same shape too:

- many files by one process: `mj_yaml_flatten_many` flattens any number of files in one awk
  (front matter only with `--front`, numbered outputs when basenames collide);
  `mj_sha256_many` hashes a list in one process; `lib/plan_json.awk` and `lib/mermaid.awk`
  build the plan's JSON and graphs in one pass over the model;
- a file into variables once: `mj_yload` and `mj_yload_dir` turn a flat file, or a
  directory of them, into shell variables with one awk and one eval; `mj_yv` and
  `mj_yvlist` read them by expansion, and the plan accessors, the catalogue, the context
  documents and the site generator read that way;
- nothing per row that forks: the tab for `IFS` is computed once (`MJ_TAB`), a substring
  replaces a `cut`, and the generator's JSON strings and arrays are built by `jstr`,
  `jarr` and `jline` instead of a jq per value.

The measured deltas of each step are in the commit messages (`git log --grep perf`), the
accepted state is [`.ai/repo/benchmarks/baseline.json`](../.ai/repo/benchmarks/baseline.json)
once one is written, and the history of local runs is under `.ai/local/benchmarks/`. This
document carries no numbers on purpose: they go stale, and the files above do not.

## Benchmarking: `majordomus bench`

`bench` times every public command of [`share/commands.yaml`](../share/commands.yaml) in a
disposable repository. Nothing keeps a list: a command added to the registry is a target
from that moment, and the harness never times itself. When the tool's own suite is present,
a target's scenario is the first scenario of its fixture under `test/fixtures/commands/`,
so what is timed is what the site demonstrates and case 34 executes; otherwise it is the
bare command in an installed repository.

Cold and warm are recorded separately and never averaged together. A read-only command is
run once cold, warmed `benchmark.warmup` times, then sampled `benchmark.samples` times in
the same repository. A command that mutates state gets a fresh repository per sample: every
sample of it is cold, and it has no warm distribution. A sample is the wall-clock time of
the child process alone; setup is never inside the clock. Every distribution carries its
count, minimum, p50, p90, p95, p99, maximum, mean and standard deviation.

```
majordomus bench                       # every target
majordomus bench doctor context        # these two
majordomus bench --list                # the targets, their class and scenario
majordomus bench --format json         # the run as one document
majordomus bench --samples 30 --warmup 5
```

The command reference is the `bench` section of [`CLI.md`](CLI.md); the document shapes
are in [`SCHEMAS.md`](SCHEMAS.md).

## Evidence, baseline, check

Three things that are easy to confuse and are kept apart:

- **A run** is local evidence: `.ai/local/benchmarks/runs/<run-id>.json`, `latest.json`,
  one line per run in `history.jsonl`. It records the commit, whether the tree was dirty,
  the platform and the profile. It is ignored by git and proves nothing about another
  machine.
- **The baseline** is the accepted state: `.ai/repo/benchmarks/baseline.json`, tracked,
  written only by `majordomus bench --write-baseline` on a clean tree (or with `--force`),
  in its own commit with the reason. It is reviewed like any other change.
- **The check** is `majordomus bench --check`: a fresh run compared with the baseline, per
  target and mode, on p50, p95 and p99; a value more than the policy's
  `benchmark.regression` fraction over the baseline is a `FAIL` naming the metric, both
  values and the threshold, and the command exits `10`. No baseline exits `12`; a baseline
  with another schema is not comparable and exits `15` rather than producing a number.

Wall-clock thresholds are noisy across machines, so the fractions in the policy are
conservative and relative; the exact evidence is the counters.

## Budgets

The policy declares a budget for the commands the hooks pay for: `benchmark.budget.doctor_ms`
and `benchmark.budget.watch_ms`. Each command reports its own wall time against its budget
at the end of its run, `INFO` under and `WARN` over, and never lets it change the exit code.
A budget is set after a run, not before; changing one is a policy edit with a run behind it.

## Policy

```yaml
benchmark:
  samples: 10            # warm samples per target
  warmup: 2              # unsampled runs before them
  regression:            # fractions over the baseline that --check refuses
    p50: 0.5
    p95: 0.5
    p99: 0.6
  budget:                # reported by doctor and watch, WARN never exit
    doctor_ms: 3000
    watch_ms: 3000
```

Every key is read with `mj_pol_req`: declared or refused, no reader-side default.

## Working on performance

1. Measure first: `MJ_TIMING=1 bin/majordomus <command>` and read the ranking.
2. Change the smallest thing that removes the repeated work; prefer one process for many
   items over a cache, because a cache is a second thing that can be believed.
3. Prove the output unchanged: the case that covers the code path, and for a generator a
   byte comparison of its output before and after.
4. Measure again, and put both numbers in the commit message.
5. Run `majordomus bench <command>` and, when the change is accepted, update the baseline
   in its own commit.

What is not done, on purpose: no cross-process result cache (the batch reads made it
unnecessary; if one is ever added it must not change any observable result and must be
proved by a case over every read-only command), no hot reload, no metrics service, no
dashboard.
