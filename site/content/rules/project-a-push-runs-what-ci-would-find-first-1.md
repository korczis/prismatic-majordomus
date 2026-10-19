+++
title = "A push runs what CI would find first"
description = "A push runs what CI would find first"
weight = 75
[extra]
kind = "rule"
slug = "project-a-push-runs-what-ci-would-find-first-1"
identity = "project.a-push-runs-what-ci-would-find-first@1"
status = "active"
source = ".ai/repo/rules/project/a-push-runs-what-ci-would-find-first.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-09 CI refused the 0.19.0 release twice, each time for something a local run finds
in seconds. The first was a structure gate, `economics-claims`, at 6 s and `always: true`.
The second was a suite case, 08, at 1 s. Nothing ran either before the push. The pre-push
hook ran `finish --check`, which reports required gates as never reported and runs none of
them, and the cases had been chosen by guess. Each refusal cost a CI round trip of more than
an hour, then another derive behind the machine's lock, on the one change the owner was
waiting for.

The whole structure job is not the answer. It takes about 20 minutes, it reaches GitHub's API,
and nine sessions push from one machine. What a push owes is the part of CI that is cheap,
local and certain to fail the same way, chosen from what CI measured.

## Required behaviour

- `.githooks/pre-push` calls `scripts/ci/run-plan --before-push` after `finish --check`. The
  policy declares it (`enforcement: structure-gates-on-push`, wired by `git-hook:pre-push`),
  so `doctor`, and through it the pre-commit hook, refuses a clone whose hook stops calling it.
- The step computes this tree's plan, as CI does, and refuses the push (exit 12) when the plan
  cannot be computed. `scripts/ci-plan` refuses a base or head that names no commit. It used
  to pipe a failing `git diff` into `sort`, whose status hid the failure, and so returned a
  plan computed from nothing with exit 0.
- **Gates.** It runs the plan's selected structure gates that are marked neither
  `network: true` nor `builds: true` in `.ai/repo/ci/gates.yaml` and whose recorded seconds are at most `MJ_PUSH_GATE_SECONDS`
  (default 15). The seconds come from `.ai/repo/ci/structure-durations.tsv`, which
  `scripts/ci-baseline --structure-durations` writes from the `ci-metrics-structure`
  artifacts CI uploads. Those seconds say which gates are cheap; they do not say what a gate
  costs here (see **Budget**).
- **Builds.** A gate marked `builds: true` compiles the crate before it measures (`cargo run`,
  `cargo test`). CI's runner has the build cached, so the seconds it records leave the build
  out. On 2026-10-15 two such gates, `skills-proof` and `economics-claims` (5 and 6 s in CI),
  made the step take 641, 1178 and 1693 s on a workstation against its bound of 180. Every
  such gate is CI's. `scripts/ci-plan` refuses a gate whose command calls cargo without the
  mark, so a new one cannot reach a push undeclared.
- **Skipped gates.** Every gate left out prints one line naming it and why: it reaches the
  network, it builds the crate, it has no recorded duration, it is over the per-gate limit, or it does
  not fit what is left of the bound. A missing durations file refuses the push.
- **Cases.** It runs the cases `.ai/repo/ci/suite-durations.tsv` records at most
  `MJ_PUSH_CASE_SECONDS` (default 5), four at a time. It leaves out, by name, a case that
  declares `majordomus-exclusive` or starts a server (`serve_up`, `mcp_shared`,
  `serve ensure|start`, the MCP launcher). Fast is what CI recorded. A server case measures
  timing and ports, and on a machine nine sessions share it fails for the load and not for
  the change: five of them did so on the first run. Every case left out is named: on a line
  of its own, or among the names of one line that counts the cases left out for one reason
  (recorded over the limit, or not recorded at all).
- **Budget.** What runs of those candidates is what fits the bound on this machine. Without
  the building gates the step still took 305-455 s on a workstation, because CI's seconds
  priced the cheap gates at ~120 s (180-280 s there) and the fast cases at a fraction of
  their 136-167 s. Each run appends what every gate and case took to the machine's file,
  `MJ_PUSH_LOCAL` (default `<git common dir>/majordomus/push-durations.tsv`, shared by the
  repository's worktrees on the machine). A candidate is priced at its last local seconds,
  or, never measured here, at CI's seconds times the ratio local/CI of its kind's measured
  candidates (`MJ_PUSH_COLD_FACTOR`, default 2, before any). The cheapest are taken first, a
  case at a quarter of its price because four run at a time, until `MJ_PUSH_BOUND` less
  `MJ_PUSH_RESERVE` (default 5) seconds, counted from the step's start, is spent; the cases
  are priced again against what the gates left, and a gate whose price no longer fits is left
  to CI rather than run past the bound. The last line counts the gates and cases left to CI.
- **Bound and verdict.** It reports its measured time against `MJ_PUSH_BOUND` (default 180).
  An overrun names the slowest steps and never refuses the push: a busy machine must not fail
  it, or people route around the step.
  A failed gate or case refuses the push, naming it and the tail of its output.
- **Stdin.** Every gate and every case runs with stdin closed. A hook's stdin is the ref
  list. `test/run.sh` gave each case the runner's stdin, because a background job under job
  control keeps it, so a case that read stdin waited for ever on an open pipe: case 54, for
  39 minutes, on the same day.

## Failure behaviour

The push is refused with the names of what failed and the sentence "CI would refuse this push
for the same reason". There is no flag that skips the step, as there is no `--no-verify`.

## Verification

`test/cases/1021_a_push_runs_what_ci_would_find_first.sh` drives the step over a plan and
durations it writes. It proves:

- a cheap gate runs, and a cheap failing gate refuses the push with its output;
- each kind of left-out gate is named with its reason: the network, a build, no recorded
  duration, over the per-gate limit, and past what is left of the bound;
- a gate is priced at what it took here, not at CI's seconds, and one never measured here at
  CI's seconds times this machine's measured ratio; what each gate took is recorded;
- over a bound of 6 s with a reserve of 2, six one-second gates priced at twice CI's seconds
  run at most two, the step's measured time stays within its bound, and every gate it does
  not run is named;
- every gate and case left to CI is named, and the last line's counts equal the names;
- every gate of the model whose command calls cargo carries `builds: true` in the plan, the
  step leaves each such structure gate of the full plan out by name, and the model refuses a
  cargo gate without the mark or with a mark that is not `true`;
- a missing durations file, or a plan that cannot be computed (an unknown base), refuses the
  push;
- the hook calls the step, the policy declares the enforcement, and the plan carries the
  model's network flag;
- a case that reads stdin under a pipe that never closes no longer hangs the runner.

The first run of the step on its own branch refused that branch for an unpinned `sort`
(order-check), before any CI saw it.
{% endraw %}
