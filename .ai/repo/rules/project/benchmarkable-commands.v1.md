---
id: project.benchmarkable-commands
version: 1
kind: rule
title: Every public command is benchmarkable from the registry
description: The benchmark targets are the public commands of share/commands.yaml and their scenarios are the command fixtures; no second list of what to time exists, and a command whose fixture the harness cannot run is a defect of the fixture, not an exemption.
statement: A public command is a benchmark target from the moment it is registered, timed through the same fixture the suite executes and the site demonstrates, with cold and warm distributions recorded separately and never averaged together.
status: active
class: advisory
depends_on: [project.no-claim-without-test@1]
tags: [performance, command]
---

# Rationale

A benchmark list kept by hand goes stale the day a command is added, which is the day the
new command's cost goes unmeasured. The registry already knows the surface, the fixtures
already describe one scenario per command, and `majordomus bench` reads both, so adding a
command adds its benchmark with no further step. Cold and warm are different facts about a
command: the first run after a fresh installation and the steady state under a warm
filesystem and shell. A single average of the two describes neither.

# Required behaviour

`majordomus bench` derives its targets from `share/commands.yaml` (every public command
except the harness), its scenarios from `test/fixtures/commands/<command>.json` when the
suite is present, and records for each target and mode the count, minimum, p50, p90, p95,
p99, maximum, mean and standard deviation. A read-only command is sampled warm in one
repository after one cold run; a command that mutates state is sampled cold in a fresh
repository per sample and has no warm distribution. Runs are local evidence under
`.ai/local/benchmarks/`; the baseline is `.ai/repo/benchmarks/baseline.json`, written only
by `--write-baseline` on a clean tree; `--check` refuses a regression over the policy's
`benchmark.regression` thresholds by exit code.

# Failure behaviour

`bench` exits `12` for a target that is not a public command and `13` when a target does
not run cleanly, naming the target and its status. Case `test/cases/79_bench_command.sh`
proves that the target list follows a registry mutation and that the distributions are
separate and complete.

# Verification

`bash test/run.sh 79_bench_command`, and `majordomus bench --list` against
`majordomus doctrine list` when a command is added.
