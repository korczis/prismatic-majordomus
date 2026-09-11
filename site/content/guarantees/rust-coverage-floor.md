+++
title = "Coverage of the Rust crate is measured with test code out of the denominator, on every change that can affect the crate and on every push to master, and the build fails under the floor in scripts/rust-coverage-threshold or under the session/continuity domain's own floor in scripts/session-coverage-threshold"
description = "The crate's coverage is a number with a committed floor, and a push that drops under the floor does not go green. Two floors, each one integer in a file of its own: scripts/rust-coverage-threshold over the whole executable, and scripts/session-coverage-threshold over the canonical session/continuity domain declared in scripts/session-coverage-domain. Both are read by every place that measures — the coverage job in CI, scripts/rust-check when cargo-llvm-cov is installed, and just coverage — so there is no second copy of either number to drift."
weight = 121
[extra]
claim_id = "rust-coverage-floor"
status = "guaranteed"
source = "docs/claims/rust-coverage-floor.md"
+++
{% raw %}

## What it means

The crate's coverage is a number with a committed floor, and a push that drops under the floor does not go green. Two floors, each one integer in a file of its own: `scripts/rust-coverage-threshold` over the whole executable, and `scripts/session-coverage-threshold` over the canonical session/continuity domain declared in `scripts/session-coverage-domain`. Both are read by every place that measures — the `coverage` job in CI, `scripts/rust-check` when `cargo-llvm-cov` is installed, and `just coverage` — so there is no second copy of either number to drift.

The measurement excludes test code. That is the part worth stating out loud, because for a long time it did not.

## How it works

`scripts/rust-coverage` runs `cargo llvm-cov --all-targets --no-fail-fast --json` once and post-processes the export. A file under `tests/`, `benches/` or `examples/`, and `build.rs`, is dropped whole; inside a `src/` file, every `#[cfg(test)]` item is dropped by line range, brace-matched from its opening line to the brace that closes it, and so is every `#[test]` and `#[bench]` function. What remains is the code under test, and the fraction is computed over that on both sides.

The previous gate was `cargo llvm-cov --all-targets --summary-only --fail-under-lines "$(cat scripts/rust-coverage-threshold)"`, with no `--ignore-filename-regex`. `--all-targets` instruments the integration suites and the benchmarks as source files of their own, and this crate keeps its unit tests inline as `#[cfg(test)] mod tests` inside `src/*.rs`, where no filename regex can reach them. Test code is nearly always fully executed by the run that measures it, so every test written raised the percentage whether or not it covered anything and every test deleted lowered it. A threshold over that mixture cannot be reasoned about, and the obvious way to hold it is to write tests of the tests.

The second floor exists because a crate-wide number says nothing about any particular subsystem. The session and continuity domain — the code a repository's memory of its own work depends on — can sit at zero while the crate-wide figure stays green. `scripts/session-coverage-domain` names the files it covers, one per line, and the script refuses a domain that names a file the export does not carry: narrowing the domain to raise the number is an edit a reviewer sees rather than a silent change of subject.

`test/cases/77_rust_evidence.sh` checks that both files hold one integer, that the coverage job runs the script rather than calling `cargo llvm-cov --summary-only` again, that `scripts/rust-check` and `just coverage` do the same, and that every file the domain names exists.

## How to see it

```bash
cat scripts/rust-coverage-threshold scripts/session-coverage-threshold
cat scripts/session-coverage-domain
scripts/rust-coverage --report        # measure and print, hold nothing
just coverage                         # cargo-llvm-cov: cargo install cargo-llvm-cov; rustup component add llvm-tools-preview
```

The report prints lines, functions and regions for the crate and for the domain, and, beside the crate's line figure, the figure the old gate measured — the same run counted with the test code left in — so that the difference between the two is visible rather than asserted.

## What it does not cover

Coverage is measured on Linux in CI, once per run that selects the `rust-coverage` gate (the `rust` and `share` classes of `.ai/repo/ci/gates.yaml`, and every full plan); a change that cannot reach the crate does not recompile it with instrumentation. Branch coverage proper is not measured: `cargo llvm-cov --branch` needs a nightly toolchain and this repository builds on stable, so region coverage is what stands in for it and the report says `regions` rather than `branches`. The crate-wide floor is on lines only; the domain floor is on lines, functions and regions together. A file under the crate floor is not refused as long as the crate as a whole is above it.

The shell half of the tool — `lib/*.sh`, which owns the session lifecycle, the ledger and the record stores — is not in this measurement and has no floor. `scripts/shell-coverage` measures it, exactly for functions and as a stated floor for lines, and is an instrument rather than a gate; the reason it is not one is in its own header.

## Why it exists

The operator asked for coverage near 100 percent and for that to be enforced for the future. A threshold that counts the tests in its own denominator answers a different question from the one that was asked: it says how much of the crate plus its tests ran, which rises when tests are added and falls when they are deleted, whatever they cover. Subtracting the test code makes the number mean what it says, and it costs something — the honest figure is lower than the one the old gate printed. The rule `project.rust-cli-evidence` states the requirement; this claim ties the numbers to the gate.
{% endraw %}
