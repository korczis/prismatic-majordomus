# RELEASE 15 · 03 Every archive builds inside seven minutes

## Operating contract
As in 00. Never weaken what `verify the artifact is what the model says it is` checks, and keep
attestations (I2165).

## Objective
From plan done to the slowest archive uploaded: at most 7 minutes. Measured today: 6-15 minutes,
the long pole being `package` on macos-x86_64 (866 s on `macos-15-intel`), then macos-aarch64
(~9 min) and linux-gnu (413 s).

## Issue 1: Profile the package step
Split `package` into toolchain, dependency compile, crate compile, strip/pack, and record each, on
each target, cold and warm. The baseline decides the remedy, not this prompt.
Acceptance: a table in the completion report and in prompt 00's record.

## Issue 2: A warm cache for release builds
Release builds start cold. Cache `target/` (or use sccache) keyed on Cargo.lock, the toolchain and
the target triple, restored on tag builds and saved by master's builds of the same configuration,
so a tag build compiles only the crate. Prove a poisoned cache cannot change the bytes: the
archive's verification and attestation run on the output as before.
Acceptance: warm and cold seconds per target; a case or CI check that the cache key covers the
inputs.

## Issue 3: macOS x86_64 from the arm runner
Build `x86_64-apple-darwin` on `macos-15` (arm, faster, more available) with the cross target,
instead of `macos-15-intel`. Smoke still runs the x86_64 archive on an x86_64 host.
Acceptance: the archive passes the same verification and smoke; seconds before/after.

## Issue 4: Build ahead of the tag
Master's push already proves the tree; build the six release archives on every master push that
declares a new version (the bump commit), keep them as workflow artifacts, and let the tag run
adopt them when the tag's commit equals the built commit (digests re-read, attested). A tag on a
commit nobody built builds as today.
Acceptance: the tag run's build phase under 2 minutes when adopted; a case for "adopted" and for
"built because the commit differs".

## Completion report
Per-target seconds before/after; which remedy carried the budget.
