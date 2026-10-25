---
id: project.a-push-runs-what-ci-would-find-first
version: 2
kind: rule
title: A push runs what CI would find first, and CI decides
description: Before a push, the pre-push hook runs the plan's structure gates that CI recorded as cheap and that reach no network, and the suite's fast cases, within a stated bound, so a failure a local run finds in seconds is seen before a CI round trip; a gate CI runs again reports and lets the push through, and only the integrity checks refuse.
statement: Push through the pre-push hook, which runs `scripts/ci/run-plan --before-push` after `finish --check` and the case-number check. A failed gate or case is named and reported, and CI holds the verdict on it. An uncomputable plan, a failed `finish --check` and a duplicate case number refuse the push. In the commit hook, `scripts/pages current`, the worktree guard and the SSH permissions check refuse and doctor reports. A gate the step leaves out is named with its reason and left to CI. Nothing skips the step.
status: active
class: blocking
depends_on: [project.the-cooperation-model-is-the-owners-word@2]
tags: [ci, process]

x-majordomus:
  tests: [test/cases/1021_a_push_runs_what_ci_would_find_first.sh, test/cases/4008_a_push_that_only_deletes_plans_nothing.sh, test/cases/3948_a_local_gate_reports_and_ci_decides.sh]
---

# Rationale

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

Version 1 refused the push on every failed gate, and on a loaded machine that made the hook
the slowest part of the work: on 2026-10-20 a push of a rescue ref spent 3490 s in the step at
load 70 to 90, and a commit hook started a cold cargo build. On 2026-10-21 the owner relaxed
the cooperation model (rule `project.the-cooperation-model-is-the-owners-word`, version 2,
I3824): local gates are informative and CI holds the verdict. A finding CI will reach again
is therefore reported here, not refused (I3830). A check CI cannot reach, or reaches only
after a merge or a deploy it would break, still refuses: it takes seconds and nothing else
would say it.

# Required behaviour

- `.githooks/pre-push` calls `scripts/ci/run-plan --before-push` after `finish --check`. The
  policy declares it (`enforcement: structure-gates-on-push`, wired by `git-hook:pre-push`),
  so `doctor` finds a clone whose hook stops calling it: the commit hook reports that finding
  and CI's core-check, which runs doctor again, refuses it.
- The step computes the push's plan, as CI does, from the commits the push sends: the
  merge base up to `HEAD` (`scripts/ci-plan --head HEAD`), never the working tree. An untracked
  file is not a change a push carries; on 2026-10-22 git's `.merge_file_*` temporaries, left at
  a checkout's root by an interrupted merge, put a push that only deleted remote refs into the
  full plan for more than 15 minutes (#2372). The step refuses the push (exit 12) when the plan
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
  budget. A missing durations file refuses the push (exit 12).
- **Cases.** It runs the cases `.ai/repo/ci/suite-durations.tsv` records at most
  `MJ_PUSH_CASE_SECONDS` (default 5), four at a time. It leaves out, by name, a case that
  declares `majordomus-exclusive` or starts a server (`serve_up`, `mcp_shared`,
  `serve ensure|start`, the MCP launcher). Fast is what CI recorded. A server case measures
  timing and ports, and on a machine nine sessions share it fails for the load and not for
  the change: five of them did so on the first run.
- **Bound.** It reports its measured time against `MJ_PUSH_BOUND` (default 180). An overrun
  names the slowest steps and never refuses the push: a busy machine must not fail it, or
  people route around the step.
- **Verdict.** A failed gate or case exits 1 with the tail of its output and a last line that
  names it and says CI holds the verdict. The hook prints `reported, not refused: CI holds the
  verdict` and lets the push through; CI's validate refuses the same head. Any other non-zero
  exit of the step, 12 above all, refuses the push: what cannot be measured is not passed.
- **Stdin.** Every gate and every case runs with stdin closed. A hook's stdin is the ref
  list. `test/run.sh` gave each case the runner's stdin, because a background job under job
  control keeps it, so a case that read stdin waited for ever on an open pipe: case 54, for
  39 minutes, on the same day.
- **Deletions.** A push whose every ref has an all-zero local sha carries no commit, so the hook
  exits 0 before `finish --check` and the plan, as it does for `refs/majordomus/*` (I4008: 16
  deletions waited 2400 s on the build lock, 2026-10-22); one ref that carries a commit and the
  whole hook applies.

# What refuses and what reports

| Hook | Check | Local verdict | Why |
|---|---|---|---|
| pre-push | `bin/majordomus finish --check` | refuses | the task's scope and contract exist only on this machine |
| pre-push | `scripts/case-numbers check` | refuses | each branch is clean alone; the pair exists only after a merge |
| pre-push | `scripts/ci/run-plan --before-push`, exit 12 | refuses | a plan that cannot be computed is not a pass |
| pre-push | `scripts/ci/run-plan --before-push`, exit 1 | reports | CI's structure job and suite run the same gates and cases |
| pre-push | `scripts/rust-check --doc` | reports | CI's rustdoc job refuses the same head |
| pre-commit | `bin/majordomus doctor` | reports | CI's core-check runs doctor and refuses |
| pre-commit | `bin/majordomus-cli worktree guard` | refuses | CI checks out a fresh clone and never sees which worktree a commit came from (rule `project.worktree-topology`) |
| pre-commit | `scripts/pages current` | refuses | a stale derive breaks the deploy for whoever lands next |
| pre-commit | `scripts/ci/providers-check --permissions-only` | refuses | the owner-authorized SSH permissions of every native client |

A reported finding prints one line on stderr, `<hook>: <check> failed (exit <n>); reported,
not refused: CI holds the verdict`, after the check's own output. The hooks carry the
`report` function themselves; there is no sourced helper.

# Failure behaviour

A refusing check exits the hook with its own code, naming what failed. A reporting check
never changes the hook's exit. There is no flag that skips any step, as there is no
`--no-verify`.

# Verification

`test/cases/1021_a_push_runs_what_ci_would_find_first.sh` drives the step over a plan and
durations it writes. It proves:

- a cheap gate runs, and a cheap failing gate exits 1 with its output and a line that names it
  and says CI holds the verdict;
- each kind of left-out gate is named with its reason: the network, no recorded duration,
  over the per-gate limit, and past the budget;
- a missing durations file, or a plan that cannot be computed (an unknown base), exits 12;
- the hook calls the step, the policy declares the enforcement, and the plan carries the
  model's network flag;
- a case that reads stdin under a pipe that never closes no longer hangs the runner.

`test/cases/4008_a_push_that_only_deletes_plans_nothing.sh`: a deletion-only push runs no
`finish`, `case-numbers` or `run-plan`, and a mixed push still runs all three.

`test/cases/3948_a_local_gate_reports_and_ci_decides.sh` runs the real hooks in a fixture
whose checks are stubs with chosen exit codes. It proves:

- a push whose step exits 1 exits 0, with the gate's line and the hook's report line;
- a push whose step exits 12, whose `finish --check` fails or whose case numbers collide is
  refused with that code;
- a push that changes the crate and whose rustdoc fails exits 0 with a report line;
- a commit whose doctor fails exits 0 with a report line, and one whose `pages current`, worktree
  guard or SSH permissions check fails is refused with that code;
- each call is still on a line doctor's wiring check matches.

The first run of the step on its own branch refused that branch for an unpinned `sort`
(order-check), before any CI saw it.
