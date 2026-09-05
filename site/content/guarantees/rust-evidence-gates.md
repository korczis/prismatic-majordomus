+++
title = "Every gate the Rust executable must pass is one script, scripts/rust-check, and CI runs that script on every change that can reach the crate"
description = "Whatever the Rust executable must satisfy before it is merged is written down once, as an executable script, and CI runs the same list in the same order. A person runs just rust-check and sees exactly what the pull request will see: cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings (a public item without documentation is a warning, so it is an error here), cargo test --no-fail-fast with the unit, integration and doc-example suites, cargo doc --no-deps with warnings denied, cargo bench --no-run, majordomus capabilities validate, majordomus generate --check, majordomus bench coverage --check, and cargo llvm-cov failing under the committed floor. CI runs one gate more, majordomus bench --profile ci --check --no-write, the benchmark check against the baseline committed for its own profile; a baseline is per platform and profile, so the script has none to check locally. Nothing in that list is advisory and nothing is skipped on a branch."
weight = 108
[extra]
claim_id = "rust-evidence-gates"
status = "guaranteed"
source = "docs/claims/rust-evidence-gates.md"
+++
{% raw %}

## What it means

Whatever the Rust executable must satisfy before it is merged is written down once, as an executable script, and CI runs the same list in the same order. A person runs `just rust-check` and sees exactly what the pull request will see: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` (a public item without documentation is a warning, so it is an error here), `cargo test --no-fail-fast` with the unit, integration and doc-example suites, `cargo doc --no-deps` with warnings denied, `cargo bench --no-run`, `majordomus capabilities validate`, `majordomus generate --check`, `majordomus bench coverage --check`, and `cargo llvm-cov` failing under the committed floor. CI runs one gate more, `majordomus bench --profile ci --check --no-write`, the benchmark check against the baseline committed for its own profile; a baseline is per platform and profile, so the script has none to check locally. Nothing in that list is advisory and nothing is skipped on a branch.

## How it works

`scripts/rust-check` is the list, one `step` per gate, in CI's order. The `rust` job in `.github/workflows/validate.yml` runs the script itself, `--ci` when the plan says the crate can be affected (every gate but coverage, plus the benchmark check) or `--integration` when only the data the registry reads changed (the executable built, `capabilities validate`, `generate --check`, `bench coverage --check`), and asks it for the executable as an artifact; the `coverage` job runs the coverage gate with the same threshold file; the `bench` job runs the benchmark check on macOS, where the committed baseline is; the `macos` job runs the crate's suites there. The justfile's `test` recipe depends on `rust-check`, so the whole suite of the repository (`just test`) includes the crate's gates. `test/cases/77_rust_evidence.sh` reads all three, the script, the workflow and the justfile, and fails when they disagree about a gate or its order, when the crate roots lose `#![warn(missing_docs)]`, or when the rule `project.rust-cli-evidence` stops being active and blocking; with a toolchain present it also runs the doc examples, builds the benchmarks and asks the built executable for the benchmark policy of every executable capability.

## How to see it

```bash
just rust-check                          # every gate, in CI's order; stops at the first failure
just test-shell 77_rust_evidence         # the wiring agrees, the doc examples pass, the benchmarks build
grep -oE 'step "[^"]+"' scripts/rust-check
awk '/^  rust:/,/^  coverage:/' .github/workflows/validate.yml | grep -- 'rust-check'
scripts/ci-plan --full x --format text | grep rust
```

## What it does not cover

The case proves the gates are wired, not that they pass on the machine running it: the lints, the full suite and the coverage measurement are what CI runs and what `just rust-check` runs, and the case runs only the two gates cheap enough to repeat, the doc examples and the benchmark build. It does not run the benchmarks or assert a latency; that is `rust-hot-path-benchmarks`. Nothing here gates a commit locally: the pre-commit hook runs `majordomus doctor`, not cargo.

## Why it exists

The operator asked that tests, doctests, benchmarks and coverage be gates for the future, enforced in CI and in the root tooling, not aspirations in a README. A gate written in a workflow file alone is one edit away from disappearing without anyone noticing; a list that the suite reads back is one a reviewer sees change. The rule `project.rust-cli-evidence` states the requirement; this claim and `test/cases/77_rust_evidence.sh` make it a fact of the tree.
{% endraw %}
