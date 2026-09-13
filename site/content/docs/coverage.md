+++
title = "Coverage: one measurement, two questions"
description = "test coverage of the Rust executable: one measurement (scripts/rust-coverage), the crate floor and the session/continuity domain, and the changed-code differential gate of #214 that refuses new debt while leaving untouched legacy debt alone"
weight = 51
[extra]
source = "docs/COVERAGE.md"
+++

{% raw %}

The Rust executable's test coverage is measured once and asked two questions. The
measurement is `scripts/rust-coverage`: one `cargo llvm-cov` export, test code excluded
from both sides of every fraction, one rule for counting lines, functions and regions
(the script's own header is the authority on how each is counted). Every consumer — the
crate gate, the domain gate, the differential gate, and any report — reads that one
judgment. There is no second threshold, no second parser, no percentage computed a
different way somewhere else.

## The two floors and the differential

**The crate floor** (`scripts/rust-coverage-threshold`, currently 90) is a lower bound on
line coverage of the whole executable, test code out of the denominator. It is a ratchet
against the crate rotting, not a promise of completeness.

**The session/continuity domain** (`scripts/session-coverage-threshold`, 100, over the
files in `scripts/session-coverage-domain`) holds one subsystem to full line, function and
region coverage. A crate-wide floor says nothing about whether the subsystem a
repository's continuity depends on is tested at all; this is where that is held.

**The differential gate** (`scripts/ci/coverage-differential`, rule
`project.new-code-is-covered`) is the changed-code invariant of issue #214. It holds every
executable line, function and region a change *adds or touches* to coverage — measured
against the branch's merge-base with the trunk, working tree included, so an uncommitted
edit is held exactly like a committed one.

## Why differential, and what it does not do

Repository-wide coverage sits below 100% and the legacy debt is real. A single crate-wide
floor cannot forbid *new* debt: a change can add uncovered code while the aggregate barely
moves. The differential gate draws the line where it can be held without rewriting history
— at the code a change touches:

<div class="overflow-x-auto" tabindex="0">

| what a change introduces | what the gate requires |
|---|---|
| a new file | every executable line, function and region covered |
| a new function | covered |
| a changed executable line | covered |
| a changed branch or region | covered |
| untouched legacy code | not this gate's business |

</div>


The invariant is `new_debt == 0` and `legacy_debt_after <= legacy_debt_before`. The gate
never looks at a line the change did not touch, so it cannot demand that old debt be paid;
it fails on the first changed line that runs and no test exercises. A changed line that
maps to nothing executable — a comment, a blank, a `use`, a type declaration, or test code
— is not a finding: there is nothing there to run, so a documentation-only or test-only
diff passes with no findings.

## Five outcomes, never conflated

The gate reports, and exits, in one vocabulary — the same `scripts/rust-coverage` uses:

<div class="overflow-x-auto" tabindex="0">

| outcome | meaning | exit |
|---|---|---|
| PASS | every changed executable item is covered | 0 |
| PASS | the change touched no crate source line | 0 |
| FAIL | a changed executable line, function or region is uncovered | 10 |
| UNKNOWN | git cannot resolve the base to diff against (a shallow clone, no remote) | 13 |
| BLOCKED | `cargo-llvm-cov` is absent, or the instrumented run could not be trusted | 12 |

</div>


Unknown is not pass. Blocked is not pass. A crashed measurement is never green — a gate
that turns "the coverage tool failed" into a pass is the failure mode the whole exercise
exists to refuse.

## Running it

```
scripts/ci/coverage-differential                 # against origin/master's merge-base
scripts/ci/coverage-differential <base>          # against an explicit base
MJ_COVERAGE_EXPORT=<file> scripts/ci/coverage-differential   # reuse an existing export

scripts/rust-coverage --changed <base>           # the measurement directly
scripts/rust-coverage                            # both floors
scripts/rust-coverage --report                   # measure, hold nothing
scripts/rust-coverage --domain                   # the session/continuity domain only
```

CI runs the gate in the `coverage` job for every change under `apps/majordomus-cli/`; a
local `check` and the pre-commit path reach the same judgment. The behavioural proof is
`test/cases/132_coverage_differential.sh`: it drives the gate against a fixed export and
synthetic diffs and asserts that a deliberately uncovered changed line fails, the same line
covered passes, a documentation-only change is held to nothing, and an unresolvable base is
UNKNOWN rather than a silent pass.

## What this is not, yet

The differential gate holds line, function and region coverage — the dimensions
`cargo-llvm-cov` measures for this crate. It does not do mutation testing or fault
injection; where a predicate's *sense* matters (an authorization check, a trust decision, a
state transition), the coverage number proves the line ran, not that a test would fail if
the line were wrong. Those subsystems carry their own adversarial tests (the mesh's trust
and protocol suites, for one), and the coverage gate is necessary, not sufficient, for
them. Coverage evidence staleness is bound to the export the gate runs against, which CI
regenerates per run; a persisted evidence-and-staleness model over recorded runs (ADR 0041
territory) is not part of this gate.
{% endraw %}
