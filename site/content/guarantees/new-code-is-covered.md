+++
title = "Every executable line, function and region a change adds or touches in the Rust crate must be covered by a test; a change that leaves new code uncovered is refused, and untouched legacy debt is left alone"
description = "Coverage across the whole crate sits below 100% and the legacy debt is real, so a single"
weight = 187
[extra]
claim_id = "new-code-is-covered"
status = "guaranteed"
source = "docs/claims/new-code-is-covered.md"
+++
{% raw %}

## What it means

Coverage across the whole crate sits below 100% and the legacy debt is real, so a single
crate-wide floor cannot forbid *new* debt: a change can add uncovered code while the
aggregate number barely moves. The differential invariant of issue #214 draws the line
where it can be held without rewriting history — at the code a change touches. A new file
owes complete coverage, a new function owes it, a changed executable line owes coverage, a
changed branch owes coverage; untouched legacy code may stay as it is. The invariant is
`new_debt == 0` and `legacy_debt_after <= legacy_debt_before`.

## How it works

`scripts/rust-coverage --changed <base>` reads the one canonical `cargo llvm-cov` export —
test code excluded from both sides of every fraction, the same line/function/region rule the
crate gate uses — and intersects it with the lines `git diff <base>` says the change added or
touched, working tree included so an uncommitted edit is held like a committed one. A changed
line that maps to executable code with a zero count is a finding; a changed line that maps to
nothing executable (a comment, a blank, a `use`, a type declaration, test code) is not.
`scripts/ci/coverage-differential` is the thin gate: it resolves the base to the branch's
merge-base with the trunk (or an explicit one), passes the verdict through, and distinguishes
PASS, FAIL (exit 10), UNKNOWN (exit 13, the base cannot be resolved) and BLOCKED (exit 12,
the coverage toolchain is absent or its run could not be trusted). The rule is
`project.new-code-is-covered`; the CI plan runs the gate in the coverage job for every change
under the crate.

## How to see it

```bash
scripts/ci/coverage-differential                          # this branch against origin/master's merge-base
scripts/rust-coverage --changed HEAD~1                    # against the previous commit
MJ_COVERAGE_EXPORT=cov.json scripts/ci/coverage-differential   # reuse an export, no reinstrument
```

`test/cases/132_coverage_differential.sh` is the executable proof: against a synthetic
self-consistent fixture it asserts a deliberately uncovered changed line FAILs (exit 10),
the same line covered PASSes, a non-crate change is held to nothing, and an unresolvable
base is UNKNOWN (exit 13) rather than a silent pass.

## What it does not cover

It holds the dimensions `cargo-llvm-cov` measures for this crate — line, function and
region. It does not mutation-test: for a predicate whose sense matters (an authorization
check, a trust decision, a state transition), it proves the changed line ran, not that a
test would fail if the line were wrong; those subsystems carry their own adversarial suites
and coverage is necessary, not sufficient, for them. It never demands that untouched legacy
debt be paid — it looks only at the lines a change touched — and a git hook run with
`--no-verify`, or a base a shallow clone cannot resolve, is UNKNOWN, not a pass it cannot
justify.

## Why it exists

A crate-wide floor lets a change add uncovered code and stay green, because one new
uncovered function barely moves a five-figure denominator; the debt accretes a commit at a
time and nobody's change is ever the one that crossed the line. Issue #214 makes the
question local — is *this* change covered — which is the only form of the question a
contributor can answer and a gate can hold without a rewrite of the crate's history.
{% endraw %}
