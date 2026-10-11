---
schema: adr/v1
id: adr-0139
kind: adr
title: Worktrees share what cargo compiles on the way, and keep their own finished artifacts
status: proposed
date: 2026-10-11
tags:
  - operations
  - worktrees
  - disk
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0055
    - file:.cargo/config.toml
    - test:test/cases/1022_worktrees_share_build_intermediates.sh
---

# 139. Worktrees share what cargo compiles on the way, and keep their own finished artifacts

## Context

ADR 0055 made disk a measured quantity and gave each worktree its own `target/`, reclaimed
by predicate once the branch has landed. That bounded the accumulation but not the
duplication: every worktree compiles the same dependencies into its own directory, and one
cold build is 2.45GiB (measured 2026-09-12). On 2026-10-11 the machine sat 2GB under the
5120MB floor that refuses a build, and the build that was refused was one more copy of
dependencies another worktree had already compiled.

Two ways of sharing were on the table.

A shared `CARGO_TARGET_DIR` is supported by the repository today and is what several
scripts already tolerate. It also shares the finished executable: whichever worktree built
last has overwritten `debug/majordomus`, so a worktree's own `mj_rust_stale` check, its
`MAJORDOMUS_BIN` borrowers and a server holding a lease all find another branch's binary
under a path that says nothing about it. That is the failure
`project.reclaim-only-what-you-own` was written about, produced by construction.

`sccache` was measured as the alternative on this machine and gave no cache hit between two
different target directories (0 hits, 28 misses over two builds of `serde` and `regex`); a
rebuild into the same directory hit. It stays optional and is not part of this decision.

Cargo's `build.build-dir` splits the two things a target directory holds: intermediates
(dependency units, fingerprints, incremental state) go to `build-dir`, and finished
artifacts go to `target-dir`. It is accepted without `-Z` by the toolchain this repository
pins (1.98.1), which is what was checked; the release that stabilised it was not.

## Decision

`.cargo/config.toml` sets `build.build-dir` to one directory under cargo's home,
`{cargo-cache-home}/build-shared/majordomus`, and does not set `target-dir`. Every worktree
therefore compiles a dependency once and keeps its own `target/` holding only its own
finished artifacts.

The location is under cargo's home because a committed file cannot hold a relative path
that is right at every worktree depth (`<repo>-wt/feat/x` and `<repo>-wt/x` differ), and an
absolute one would name a person's machine.

The directory belongs to no worktree, so the per-worktree predicates of ADR 0055 cannot
classify it. `scripts/reap-orphans` reads it from the checkout's own declaration, measures
it with the worktrees, bounds it at 6144MB (`MAJORDOMUS_SHARED_BUILD_CAP_MB`, two cold
builds) and, under `--reclaim`, removes it as a whole when it is over its bound and no
`cargo` or `rustc` process is running. It is regenerable, so the cost of a wrong removal
is one dependency rebuild and cannot be a byte of authored source. A checkout that
declares no `build-dir` has nothing reclaimed, so a fixture or an adopter is never acted
on by a default.

## Alternatives rejected

- **A shared `CARGO_TARGET_DIR`.** Shares the executable, as above.
- **sccache as the mechanism.** No cross-directory hit was measured; it adds a daemon and
  a second cache to bound.
- **A `target-dir` symlink per worktree.** The ownership test of ADR 0055 refuses to treat a
  symlinked directory as the worktree's own, correctly, so the reaper would never touch it.
- **A policy key that generates `.cargo/config.toml`.** Better in the end — one
  declaration under `.ai/` and a generated projection — and a Rust change to the
  generator; it is the follow-up, and the config file carries the one line meanwhile.

## Consequences

- A second worktree's cold build reuses the first one's dependencies: measured 11s to 1s on
  a two-dependency fixture (`test/cases/1022`). The repository's own saving has not been
  measured and is the number to read after a week of use.
- Dependencies are shared. Whether the workspace crate is also compiled once per worktree
  was not measured; it is expected to be, since a path source's identity includes its path.
- Builds that share the directory serialise on cargo's file lock: two worktrees building
  at once wait for each other. That is the cost, and `derive` already runs one at a time.
- Intermediates are in one place, so `cargo clean` in a worktree cleans its artifacts and
  not the shared directory, which is what the ownership rule wants.
- The shared directory is outside every checkout, so removing a worktree never frees it;
  only the reaper's bound does. That is the new accumulation and its measure, threshold
  and reducer are named in `project.worktrees-share-build-intermediates`.
- Proven by `test/cases/1022_worktrees_share_build_intermediates.sh`.
