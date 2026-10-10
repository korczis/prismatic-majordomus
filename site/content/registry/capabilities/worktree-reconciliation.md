+++
title = "worktree.reconciliation"
description = "Every non-trunk branch and every detached worktree with the one state git and the kernel support for it — active (a process works inside), dirty (files no commit carries), unpublished (a commit no remote holds), orphaned (a detached worktree is its commit's only name), unstarted, merged (the trunk reaches it), equivalent (merging it changes nothing), conflicted, stale, ready — the readings that decided it, the one step it permits and the command that takes it, and whether `majordomus worktree reconcile --apply` would take that step on its own. Decided against the remote-tracking branch the trunk follows, never from an age or a name. A read: removing anything is the command line's `--apply`, which measures each subject again first."
weight = 183
slug = "worktree-reconciliation"
[extra]
id = "worktree.reconciliation"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
