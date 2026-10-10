+++
title = "A push runs what CI would find first"
description = "A push runs what CI would find first"
weight = 66
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
- **Gates.** It runs the plan's selected structure gates that are not marked `network: true`
  in `.ai/repo/ci/gates.yaml` and whose recorded seconds are at most `MJ_PUSH_GATE_SECONDS`
  (default 15). The seconds come from `.ai/repo/ci/structure-durations.tsv`, which
  `scripts/ci-baseline --structure-durations` writes from the `ci-metrics-structure`
  artifacts CI uploads. It takes them cheapest first while their sum stays within
  `MJ_PUSH_GATE_BUDGET` (default 120).
- **Skipped gates.** Every gate left out prints one line naming it and why: it reaches the
  network, it has no recorded duration, it is over the per-gate limit, or it is past the
  budget. A missing durations file refuses the push.
- **Cases.** It runs the cases `.ai/repo/ci/suite-durations.tsv` records at most
  `MJ_PUSH_CASE_SECONDS` (default 5), four at a time. It leaves out, by name, a case that
  declares `majordomus-exclusive` or starts a server (`serve_up`, `mcp_shared`,
  `serve ensure|start`, the MCP launcher). Fast is what CI recorded. A server case measures
  timing and ports, and on a machine nine sessions share it fails for the load and not for
  the change: five of them did so on the first run.
- **Bound and verdict.** It reports its measured time against `MJ_PUSH_BOUND` (default 180). An overrun names the slowest steps and never refuses the push: a busy machine must not fail it, or people route around the step.
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
- each kind of left-out gate is named with its reason: the network, no recorded duration,
  over the per-gate limit, and past the budget;
- a missing durations file, or a plan that cannot be computed (an unknown base), refuses the
  push;
- the hook calls the step, the policy declares the enforcement, and the plan carries the
  model's network flag;
- a case that reads stdin under a pipe that never closes no longer hangs the runner.

The first run of the step on its own branch refused that branch for an unpinned `sort`
(order-check), before any CI saw it.
{% endraw %}
