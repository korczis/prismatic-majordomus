# The Rust executable the rust gate built is published as an artifact with its provenance, and every Rust case drives the executable MAJORDOMUS_BIN names instead of building one

## What it means

The debug executable the `rust` job builds does not vanish with the runner: it is the artifact `majordomus-cli-<target>`, with a `majordomus-cli.json` beside it naming the commit, the target triple, the toolchain and the `Cargo.lock` digest, so a later job or a later phase (a release, a scenario runner over the real binary) can download it, set `MAJORDOMUS_BIN`, and never run `cargo build`. Every case that drives the executable honours that variable, as the launcher `bin/majordomus-mcp` already did.

## How it works

`scripts/rust-check --artifact DIR` copies `target/debug/majordomus` into `DIR` and writes the metadata; the workflow uploads the directory. `rust_bin` in `test/lib.sh` is what a Rust case calls first: with `MAJORDOMUS_BIN` set it hands the path back and never builds, refusing a path that is not executable as a failure rather than a skip; unset, it builds once with cargo and hands back the target path; with neither cargo nor the variable it returns the skip code, which the cases turn into the skip they always had. The suite and macOS jobs build once and export the variable, so no case builds. `test/cases/94_ci_plan.sh` proves the three answers of `rust_bin`; `test/cases/72_rust_mcp.sh` and the other Rust cases run through it.

## How to see it

```bash
scripts/rust-check --integration --artifact /tmp/dist && cat /tmp/dist/majordomus-cli.json
MAJORDOMUS_BIN=/tmp/dist/majordomus bash test/run.sh 72_rust_mcp
MAJORDOMUS_BIN=/nonexistent bash test/run.sh 72_rust_mcp     # a failure, not a skip
just test-shell 94_ci_plan
```

## What it does not cover

Two cases keep cargo for the half of their assertions that is cargo's own (the crate's HTTP and projection suites in 76, the doc examples and the benchmark build in 77); with the executable supplied and no cargo they run their executable half and say what they skipped. Within one workflow run the suite does not wait for the `rust` job's artifact: measured, a warm-cache build costs seconds while the wait would serialise minutes (`docs/CI.md`). The artifact is the debug profile, which is what every gate drives; release binaries are a release workflow's.

## Why it exists

Later phases of the pipeline need the executable, and a job that rebuilds it casually pays the whole compile again on an isolated runner. Publishing the build once with enough provenance to know it is the right one, and teaching every consumer one variable, makes the executable a first-class build output rather than a side effect of the tests.
