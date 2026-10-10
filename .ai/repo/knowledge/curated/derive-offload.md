---
schema: knowledge/v1
id: derive-offload
kind: knowledge
class: lesson
title: A derive can run on an idle machine and come back staged, by a script that lives outside this repository
description: Why scripts/derive is offloaded on a loaded machine, how the current script does it, its traps, and the typed capability that is owed instead.
status: verified
epistemics: observed
date: 2026-10-10
tags:
  - derive
  - performance
  - cluster
  - integration
provenance:
  origin: authored
  derived_from:
    - file:scripts/derive
    - file:docs/MESH.md
relations:
  - type: relates_to
    target: file:.ai/repo/workflows/landing-train.md
  - type: relates_to
    target: file:.ai/repo/rules/project/one-derive-at-a-time.v1.md
---

# Derive offload

`scripts/derive` is what every change here waits for: the commit hook refuses a tree that is
not derived, and its site-data stage dominates the time. Measured on 2026-10-10: about 5 to
10 minutes on an idle Apple M4 Pro, 699 s on lundra (Linux, 8 cores, load 3, the build
included), and 50 to 65 minutes on the shared MacBook at load 25 with nine sessions deriving
in turn behind one lock.

The current practice is a script outside the repository, `~/dev/.derive-offload.sh
<worktree> [user@host]`, defaulting to lundra over the tailnet (the hosts are those of the
trust row in `docs/MESH.md`). It:

- ships the worktree as a git bundle over ssh: `HEAD` when it is clean, a `git stash create`
  snapshot when it is not, and, for a merge in progress with every conflict staged, a scratch
  commit written with `git commit-tree` with both parents, so that the changelog — composed
  from `git log` — sees the merged history. A snapshot keeps only one parent, and a derive of
  it was refused by CI (`generate --check`: changelog and artifacts stale);
- derives on the host in a detached worktree `offload-<sha10>` under a host-wide lock, with
  one `CARGO_TARGET_DIR` shared by every offloaded derive on that host;
- brings the derived files back and stages them in the worktree. It commits nothing, pushes
  nothing, starts no CI and runs no hook; the commit that follows is judged by the hook as
  usual. A derive that deletes a tracked file is refused rather than half-applied.

Traps met the same day:

- Untracked files do not travel: `git add` new files first. The script refuses otherwise.
- A host whose `~/.cargo/config.toml` sets `-C lto=thin` in `rustflags` (the second MacBook)
  breaks the debug build the derive needs; derive there with `RUSTFLAGS=''`.
- Never edit the script while an instance runs: bash reads a running script from a byte
  offset, and an edit made 50 s after another session started it changed what that run
  executed. Check with `pgrep -fl derive-offload.sh`, and run a private copy.
- One target directory per host, not one per worktree: each is 4 to 8 GiB, and twelve filled
  the second MacBook's disk to 2.9 GiB free in an afternoon.
- The host's lock is not first-come-first-served, and every derive builds the executable
  first; on lundra that is about 5 of the 12 minutes.
- The output is derived on the host's platform. Linux is the platform CI judges on.

What is owed: new repository automation is a typed or scripted capability, not shell (ADR
0069 as `docs/SHELL.md` and `.ai/repo/automation/README.md` cite it), and this script is
shell, kept beside the repository rather than in it. A typed
offload — host selection from the mesh, the bundle exchange, the lock and the staging as one
declared capability with its refusals tested — is the gap. Until it exists this note
describes the practice; it does not endorse the script as the mechanism.
