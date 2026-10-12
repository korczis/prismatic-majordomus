---
schema: adr/v1
id: adr-0143
kind: adr
title: A derive leaves stage B alone only when nothing it can read changed
status: proposed
date: 2026-10-12
tags:
  - derivation
  - performance
provenance:
  origin: extracted
  derived_from:
    - file:scripts/derive
    - file:apps/majordomus-cli/src/derive/mod.rs
    - test:test/cases/1100_a_derive_skips_only_what_did_not_change.sh
---

# 143. A derive leaves stage B alone only when nothing it can read changed

## Context

`scripts/derive` runs four stages on every call, and stage B — `scripts/generate-site-data`
— is nearly all of its time. Measured on this machine on 2026-10-12 at load 25-30: a full
derive 546 s, of which stage B 512 s; a derive after a test-only change 415 s, with 0 files
changed. Nothing let stage B notice that its inputs had not moved.

The generator does record an input hash (`source_hash`), and it is the obvious key. It is
not a sound one. Its list leaves out the generator's own text, every sourced library but
one, its awk and jq programs, `site/content-src/`, the release records, the closed sessions,
and the executable its scenarios run. A skip keyed on it would keep stale output after any
of those moved, and nothing would say so until a full check ran.

## Decision

An opt-in mode of `scripts/derive` leaves stage B alone as a whole when, and only when:

1. the tree has a key at all: no untracked, non-ignored file, no unmerged path, no tracked
   symlink or submodule, a share directory inside the tree, and scenarios that would run the
   very executable deciding;
2. the key of the tree equals the key of this checkout's last successful stage B;
3. every file under the outputs the generator declares (`--outputs`) is on disk with the
   digest that run left, and nothing else is there;
4. no run was left unfinished.

The key is a superset of what stage B reads rather than a list of it: every tracked path
with its work-tree bytes and mode, except the stage's own outputs (by name, because links
resolve against the tracked list) and the crate's tests and benches (by presence, because
only `cargo test` reads them); every ignored file a glob could reach; the executable's
generation, profile and target; the tools on `PATH`; the `MAJORDOMUS_*`, `MJ_*` and locale
environment; git's global and system configuration; and the local use-case evidence that
`usecase coverage` reads into committed output. An input nobody listed is in it by
construction. The cost is skips, never correctness.

The decision and the record are typed Rust (`apps/majordomus-cli/src/derive/`), reached
through `majordomus derive begin|record`. The script keeps the arguments, the preflight,
the lock, the progress lines and the exit codes. The record lives under the checkout's git
directory, is written only after stage B exited 0 and nothing it could read moved while it
ran, and is dropped before a stage B begins that will replace what it describes.

`--full` and the default are the derive as it was. `--shadow` decides the same way, runs
stage B regardless, and exits 13 after the last stage when the run changed a byte the skip
would have kept. `scripts/derive-check`, `scripts/pages current`, the release job and the
version job never consult the record; the two workflows pass `--full`.

## Alternatives rejected

- Keying on `source_hash`. Unsound for the reasons above; widening its list by hand would be
  a second inventory that falls behind the generator the day a section is added.
- Splitting stage B now (scenarios apart from projection). It is where the larger saving is,
  and it needs the narrower input set verified first (a syscall trace, the pack's page 07).
- A content-addressed store and restore. Without it a skip needs the outputs already on
  disk, which is the case this slice serves: the repeated derive of one checkout.

## Consequences

- A repeated derive of an unchanged checkout under `--incremental` pays the build, stages
  A, C and D, and two key measurements instead of stage B.
- Any tracked edit outside the two exemptions reruns stage B, including an edit to a test
  case under `test/`. Narrowing that is the next slice.
- After a change whose stage C rewrites an input of stage B, the next incremental derive
  runs stage B once more before it settles: the record describes what stage B read, and
  stage C moved it.
- Making incremental the default waits for a shadow window with no divergence.
