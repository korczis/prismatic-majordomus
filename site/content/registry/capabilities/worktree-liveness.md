+++
title = "worktree.liveness"
description = "The one liveness predicate every deleting sweep asks (I4346): `worktree cleanup --remove` in process, `scripts/reap-orphans --targets` and the build host's `disk-guard` through `majordomus worktree liveness <path>...`. Each path is judged with the work tree holding it, and is `keep` when an open task is recorded in the tree, a live mesh claim or active session names its issue, branch or path, a peer is attached to or announced on the tree's own server, the derive lock's live owner names its branch, an offload run record or offload lock pid file names a process working in it, a CI runner job runs under the home holding it, another process's `CARGO_*_DIR` or `MAJORDOMUS_BIN` points into it, another process works inside it or names it on its command line, or the tree's index, `HEAD` or `HEAD` reflog changed within the idle window (thirty minutes unless `idle` says otherwise). Every reason is reported. A reading that could not be taken keeps the path (`unreadable`); a source that could not be read at all — the process table, the working directories, this user's process environments, the claims of a repository that declares a mesh, the offload run records — is named in `blind`, and the answer is then `complete: false`, on which the command line exits 10 and a sweep removes nothing. Writes nothing and refreshes no index."
weight = 227
slug = "worktree-liveness"
[extra]
id = "worktree.liveness"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
