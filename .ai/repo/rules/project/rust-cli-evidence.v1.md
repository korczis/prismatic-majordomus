---
id: project.rust-cli-evidence
version: 1
kind: rule
title: The Rust executable carries its evidence with it
description: Every capability and every documented behaviour of apps/majordomus-cli is proved by a behavioural test, every public item is documented with an executable example where one is meaningful, line coverage stays above the threshold CI enforces, and the hot paths carry benchmarks.
statement: A behaviour of the Rust executable exists when a test proves it, a public item exists when its documentation says what it does, and the coverage threshold and the benchmarks are gates, not aspirations.
status: active
class: blocking
depends_on: []
tags: [rust, testing, documentation]
---

# Rationale

The executable is the part of Majordomus other programs talk to, so a claim about it that no test proves and a public item nobody documented are exactly the drift the shell tool's `no-claim-without-test` rule exists to prevent. Doc examples that compile and run (`cargo test --doc`) keep the documentation true the way a behavioural case keeps a claim true.

# Required behaviour

Under `apps/majordomus-cli/`: `#![warn(missing_docs)]` stays on and clippy runs with warnings as errors, so an undocumented public item does not build in CI; public functions whose use is not obvious carry a doctest; every command, option, projection and diagnostic code named in `README.md` or `docs/CAPABILITIES.md` is exercised by a test under `tests/` or a case under `test/cases/`; `scripts/rust-check` runs the same gates CI runs, including `cargo llvm-cov` with the threshold CI enforces, and `benches/` measure the paths that scale with the repository (the YAML subset, discovery, the index, the registry, the OpenAPI document, one MCP listing).

# Failure behaviour

CI fails on a missing doc, a failing doctest, a coverage figure under the threshold, or a stale generated projection; a reviewer refuses a documented behaviour with no test naming it.

# Verification

`test/cases/77_rust_evidence.sh`: the wiring, read from the crate roots, `scripts/rust-check`, the threshold file, the CI workflow and the justfile, which must agree on every gate and its order; with a toolchain present it also runs the doc examples, builds the benchmarks and reads the benchmark policy of every executable capability back through the built executable. The gates themselves: `scripts/rust-check` (`just rust-check`), and the `rust` and `coverage` jobs in `.github/workflows/validate.yml`. Claims `rust-evidence-gates`, `rust-coverage-floor` and `rust-hot-path-benchmarks` in `docs/CLAIMS.yaml`.
