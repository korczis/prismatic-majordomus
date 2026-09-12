---
id: project.new-code-is-covered
version: 1
kind: rule
title: New and changed executable code is covered by a test; legacy debt may only shrink
description: Every executable line, function and region a change adds or touches in the Rust crate must be covered by a test, measured over the same export and test-exclusion as the crate gate; untouched legacy debt is out of scope, and no change may add debt.
statement: Every executable line, function and region a change adds or touches under apps/majordomus-cli/src must be covered by a test; the differential gate measures this against the branch's merge-base with the trunk, working tree included, and refuses a changed line that runs and no test exercises — new debt is forbidden, legacy debt on untouched lines is not this gate's business.
status: active
class: blocking
depends_on: []
tags: [coverage, testing, doctrine]

x-majordomus:
  tests: [scripts/ci/coverage-differential, test/cases/132_coverage_differential.sh]
---

# Rationale

Repository-wide coverage sits below 100% and the legacy debt is real; a single crate-wide
floor cannot forbid new debt, because a change can add uncovered code and the aggregate
number barely moves. #214 draws the line where it can be held without rewriting history:
the code a change *touches*. A new file owes 100%, a new function owes 100%, a changed
executable line owes coverage, and a changed branch owes coverage. Untouched legacy code
may stay uncovered — `legacy_debt_after <= legacy_debt_before` — but a change may add none.

The measurement is not a second opinion. It is the same `cargo llvm-cov` export, the same
exclusion of test code from both sides of the fraction, and the same line/function/region
rule the crate gate uses (`scripts/rust-coverage`); the differential is that judgment
intersected with the lines git says the change touched. There is one coverage authority, and
this rule is a narrower question asked of it — not a parallel calculation that could
disagree.

# Required behaviour

1. Every executable line, function and region that a change adds or touches under
   `apps/majordomus-cli/src/*.rs`, measured against the branch's merge-base with the trunk
   (working tree included), is covered by a test.
2. A changed line that maps to no executable code — a comment, a blank, a `use`, a type
   declaration, test code — is not held: there is nothing there to run, so a documentation
   or test-only diff passes with no findings.
3. The differential is measured over the one canonical export and the one test-exclusion of
   `scripts/rust-coverage --changed`; no consumer computes coverage a second way.
4. The gate distinguishes PASS, FAIL, UNKNOWN (the base cannot be resolved) and BLOCKED (the
   coverage toolchain is absent or its run could not be trusted). Unknown is not pass;
   blocked is not pass; a crashed measurement is never green.

# Failure behaviour

`scripts/ci/coverage-differential` exits 10 naming every changed executable line, function or
region that no test covers, with its file and line; the `coverage-differential` gate in
`.ai/repo/ci/gates.yaml` runs it for every change under the crate. It exits 13 when git
cannot resolve the base (a shallow clone, no remote) and 12 when `cargo-llvm-cov` is missing
or the instrumented run failed — each a refusal to certify, never a pass. What a static
reading cannot see — that the measurement rule is the crate gate's own — is held by the
behavioural case named below, which plants a deliberately uncovered changed line and proves
the gate fails, then covers it and proves the gate passes.

# Verification

`scripts/ci/coverage-differential` (the gate, exit 10 on an uncovered changed item) and
`test/cases/132_coverage_differential.sh`, which drives `scripts/rust-coverage --changed`
against a fixed llvm-cov export and a fixed diff: it asserts an uncovered changed line
FAILs (exit 10), a covered changed line PASSes (exit 0), a documentation-only change is held
to nothing, and an unresolvable base is UNKNOWN (exit 13) rather than a silent pass.
