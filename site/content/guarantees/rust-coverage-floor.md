+++
title = "Line coverage of the Rust crate is measured on every change that can affect the crate and on every push to master, and the build fails under the floor in scripts/rust-coverage-threshold, one integer read by CI, by scripts/rust-check and by just coverage"
description = "The crate's line coverage is a number with a committed floor, and a push that drops under the floor does not go green. The floor is one integer in scripts/rust-coverage-threshold, read by every place that measures: the coverage job in CI, scripts/rust-check when cargo-llvm-cov is installed, and just coverage. There is no second copy of the number to drift."
weight = 109
[extra]
claim_id = "rust-coverage-floor"
status = "guaranteed"
source = "docs/claims/rust-coverage-floor.md"
+++
{% raw %}

## What it means

The crate's line coverage is a number with a committed floor, and a push that drops under the floor does not go green. The floor is one integer in `scripts/rust-coverage-threshold`, read by every place that measures: the `coverage` job in CI, `scripts/rust-check` when `cargo-llvm-cov` is installed, and `just coverage`. There is no second copy of the number to drift.

## How it works

`cargo llvm-cov --all-targets --summary-only --fail-under-lines "$(cat scripts/rust-coverage-threshold)"` instruments the library, the binary, every integration suite and the doc examples, and exits non-zero under the floor. `test/cases/77_rust_evidence.sh` checks that the file holds one integer between 90 and 100, that the coverage job in `.github/workflows/validate.yml` reads that file, and that the script and the justfile do the same. The floor is 90; the measured figure at the time the floor was set was just above it, so the floor is a floor, not a target already left behind.

## How to see it

```bash
cat scripts/rust-coverage-threshold
just coverage                            # cargo-llvm-cov: cargo install cargo-llvm-cov; rustup component add llvm-tools-preview
cd apps/majordomus-cli && cargo llvm-cov --all-targets --summary-only | tail -3
```

## What it does not cover

Coverage is measured on Linux in CI, once per run that selects the `rust-coverage` gate (the `rust` and `share` classes of `.ai/repo/ci/gates.yaml`, and every full plan: master, the schedule, a dispatch, a pull request labelled `ci:full`); a change that cannot reach the crate does not recompile it with instrumentation. The case does not measure it, because a measurement takes minutes and belongs in one place. The floor is on lines, not on regions, functions or branches. A file under the floor is not refused as long as the crate as a whole is above it; the summary names the files, and raising the floor is how the bar moves. Lowering it is an edit to the file and to the case, so it is visible, but nothing prevents a reviewer from accepting it.

## Why it exists

The operator asked for coverage near 100 percent and for that to be enforced for the future. A threshold that only a workflow file knows is a number nobody reads until it is gone; one file that three tools read and one case checks is a number a reviewer sees. The rule `project.rust-cli-evidence` states the requirement; this claim ties the number to the gate.
{% endraw %}
