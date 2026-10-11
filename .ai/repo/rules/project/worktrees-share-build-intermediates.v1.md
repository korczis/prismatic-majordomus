---
id: project.worktrees-share-build-intermediates
version: 1
kind: rule
title: Worktrees share what cargo compiles on the way, and never the finished artifact
description: This repository's .cargo/config.toml names one build directory for the intermediates of every worktree and no target directory; the shared directory is measured, bounded and reduced by scripts/reap-orphans, and a worker neither points CARGO_TARGET_DIR at a directory another checkout builds into nor removes the shared directory by hand.
statement: Cargo intermediates live in one declared build-dir shared by every worktree; finished artifacts stay in each worktree's own target; the shared directory is bounded by scripts/reap-orphans and removed only as a whole, when over its bound and nobody is building.
status: active
class: blocking
depends_on: [project.worktree-topology@1, project.reclaim-only-what-you-own@1, project.accumulation-is-measured@2, project.destructive-sweeps-fail-closed@1]
tags: [operations, worktrees, disk, build]

x-majordomus:
  tests: [test/cases/1022_worktrees_share_build_intermediates.sh, test/cases/276_disk_is_bounded.sh]
---

# Rationale

Every worktree of this repository compiled the same dependencies into its own `target/`,
2.45GiB apiece (ADR 0055), and the disk those copies hold is what stopped a build on
2026-10-11. The obvious saving — one shared `CARGO_TARGET_DIR` — also shares the finished
executable, so a build in one worktree replaces the binary another is running or borrowing.
That is the incident `project.reclaim-only-what-you-own` records, produced by construction.

Cargo keeps the two apart: `build.build-dir` holds intermediates, `build.target-dir` holds
finished artifacts. Sharing the first and not the second gives the saving without the
shared executable (ADR 0139).

# Required behaviour

**`.cargo/config.toml` sets `build.build-dir` and does not set `build.target-dir`.** The
directory is under cargo's home, so it is one path at every worktree depth. A committed
`target-dir`, or a `CARGO_TARGET_DIR` that two checkouts build into, is the shared
executable again. Running another checkout's executable through `MAJORDOMUS_BIN` stays
allowed; it is building into its directory that is not.

**The shared directory is accumulation, so it has a measure, a threshold and a reducer.**
`scripts/reap-orphans --targets` measures it with the worktrees; it is bounded at 6144MB
(`MAJORDOMUS_SHARED_BUILD_CAP_MB`); `--reclaim` removes it as a whole when it is over that
bound and no `cargo` or `rustc` process is running.

**The sweep acts on a declaration and never on a guess.** The directory is read from the
checkout's own `.cargo/config.toml`; a checkout that declares none has nothing reclaimed.

**A worker does not delete the shared directory by hand while a build runs anywhere.** It is
regenerable, but a build that loses it mid-run fails for a reason nobody can see from that
build.

# Failure behaviour

A shared directory that cannot be measured, or that carries no mark of cargo's, is skipped
and counted, not removed (`project.destructive-sweeps-fail-closed`). A liveness reading that
comes back empty reclaims nothing at all. If another worktree's build holds cargo's file
lock, a second build waits for it; that is the cost of sharing and not a fault.

# Verification

`test/cases/1022_worktrees_share_build_intermediates.sh` holds the declaration (a
`build-dir`, no `target-dir`), proves on a two-root fixture that the second root does not
recompile the shared dependency and keeps its own executable, and drives the reaper through
within-bound, over-bound, build-running, idle and undeclared. `test/cases/276_disk_is_bounded.sh`
holds the per-worktree sweep this extends.
