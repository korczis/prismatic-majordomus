# RELEASE 15 · 02 The publish job derives once and builds nothing

## Operating contract
As in 00. release.yml is shared: I2302 (b4) changes the staging line of the publish job, I2303
adds a step after it. Read both branches; stack on whichever has landed; announce first.

## Objective
The publish job takes at most 3 minutes from the last archive to the metadata on gh-pages.
Measured today: 8-13 minutes.

## Issue 1: One derivation, judged without re-deriving
Today the job runs `scripts/derive` (284 s) and then `scripts/derive-check` (271 s), which derives
the same tree again to compare. Make the judgement read the first derivation's output: either
`scripts/derive --check-after` writes and proves in one pass, or `derive-check` gains a mode that
compares the staged tree with the outputs the derive just recorded (hashes in the derivation
graph). The proof must still fail when a projection is stale.
Acceptance: mutation case (a stale projection is refused); measured step seconds before/after.

## Issue 2: The executable comes from the build, not from cargo
The job rebuilds the executable (117 s) the linux-x86_64-gnu build job already packaged. Download
that archive (it is in `dist/` already), verify its digest against what the build recorded,
extract it, and use it as `MAJORDOMUS_BIN`. Fall back to `cargo build` only when the archive is
absent, and say so.
Acceptance: the job's log names the archive it used; case 87 holds the order.

## Issue 3: The derivation of a record is incremental
A release record changes `.ai/repo/releases/<tag>.yaml` and the projections that read it. The
derivation graph (ADR on derive-follows-every-canonical-input) knows which outputs depend on it:
regenerate only those, in dependency order. Measure.
Acceptance: the publish job's derive under 60 s on the runner; `derive-check` proves the whole
tree current.

## Issue 4: Checkout and setup cost
Checkout 76 s with fetch-depth 0: decide what history the job needs (the changelog reads
`git log`), fetch that with `filter: blob:none`, and cache the Rust toolchain.
Acceptance: measured.

## Completion report
Step seconds before/after for v-next; the publish job's total.
