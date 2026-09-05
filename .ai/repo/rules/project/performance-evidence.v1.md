---
id: project.performance-evidence
version: 1
kind: rule
title: A performance claim carries its measurement
description: A change made for speed names the phase or counter it changed and both values, measured on this repository with the tool's own timing; a budget or a threshold is set from a run, never guessed; a baseline is a tracked file written by the harness and updated on purpose.
statement: No speed-up without numbers, no budget without a run behind it, no baseline update without a reviewable diff; observed values live in the benchmark history and the baseline file, never in prose.
status: active
class: advisory
depends_on: [project.no-counts-in-prose@1]
tags: [performance, evidence]
---

# Rationale

A speed-up without a measurement is a claim, and a claim about performance decays faster
than most: the next change silently takes it back. The tool has the instruments to make
every such claim checkable: `MJ_TIMING=1` ranks the phases of one run, `majordomus bench`
records distributions, `--write-baseline` accepts a state and `--check` holds the next
run to it. A number written into a document is the one place none of those instruments
reach, which is why observed values do not live there.

# Required behaviour

A commit that changes performance states the phase or counter, the value before and the
value after, as measured with `MJ_TIMING=1` or `majordomus bench` on this repository. A
budget in the policy (`benchmark.budget.*`) and a threshold (`benchmark.regression.*`) is
set after a run and named with the run that justified it. The baseline changes only through
`majordomus bench --write-baseline`, in its own commit with the reason
(`perf(baseline): ...`). Documents explain the mechanism and point at the baseline file,
the benchmark history and the timing switch for the numbers.

# Failure behaviour

No command decides this rule; a reviewer does, and a performance change without its
measurement, or a baseline edit by hand, is not merged. `project.no-counts-in-prose` is
the rule that keeps the numbers out of the documents.

# Verification

Review of the commit message against `MJ_TIMING=1 bin/majordomus <command>` and of the
baseline diff against `majordomus bench --check`.
