---
id: project.rust-benchmark-coverage
version: 1
kind: rule
title: No endpoint of the Rust executable without a benchmark, and no number without evidence
description: Every externally callable operation of the Rust executable (each executable capability directly and on every transport its exposure declares, and the transports' own operations) is a benchmark target with a denominator generated from the registry; coverage is complete or the structural check fails; a waiver is typed, reported and never counted; and a claim about latency is made from a recorded measurement, never from prose.
statement: An executable capability exposed on a transport is a benchmark target there, with a case its input type provides; the coverage denominator is computed from the registry and never written down; a missing case fails majordomus capabilities validate, bench coverage --check and CI; and no latency figure appears in a document that a run under .ai/local/benchmarks/ or a baseline under .ai/repo/benchmarks/rust/ does not carry.
status: active
class: blocking
depends_on: []
tags: [rust, performance, testing]
---

# Rationale

"Representative endpoints only" is how the slow one escapes. The benchmark projection
derives every target from the same declaration that derives the endpoint, so an operation
cannot exist without its measurement, and a measurement cannot outlive the operation it
measured. The baseline is one reviewable file per platform and the regression policy is
data, so a threshold is a decision somebody made in a commit.

# Required behaviour

A capability's input type implements `BenchmarkCases` and produces at least one case for
the repository at hand; `majordomus bench coverage` reports `missing 0`; `majordomus bench
baseline update` is the only way a baseline changes, from a clean tree unless
`--allow-dirty` is said; `majordomus bench --check` compares a run with its platform's
baseline under `.ai/repo/benchmarks/rust/policy.yaml` and reports every line, new targets
and stale baseline entries by name.

# Failure behaviour

`capabilities validate` exits 10 naming the missing requirement; `bench coverage --check`
exits 10; `bench --check` exits 10 on a regression over the policy; CI runs both checks;
a document that states a latency without pointing at evidence is refused in review.

# Verification

`apps/majordomus-cli/tests/{bench,bench_units}.rs`, `test/cases/91_canonical_architecture.sh`,
the `rust` job in `.github/workflows/validate.yml`.
