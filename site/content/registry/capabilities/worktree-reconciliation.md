+++
title = "worktree.reconciliation"
description = "Every non-trunk branch and every detached worktree with the one state git and the kernel support for it — active (a process works inside), dirty (files no commit carries), unreadable (a reading failed, so nothing is decided), unpublished (a commit no remote holds), orphaned (a detached worktree is its commit's only name), unstarted, merged (the trunk reaches it), equivalent (merging it changes nothing), conflicted, stale, ready — the readings that decided it, the one step it permits, the command a person runs to take that step, and whether it is removable on proof alone. Decided against the remote-tracking branch the trunk follows, never from an age or a name. A read: it removes nothing, and a removal it names is git's own guarded form (`git worktree remove` without `--force`, `git update-ref -d` on the judged commit)."
weight = 199
slug = "worktree-reconciliation"
[extra]
id = "worktree.reconciliation"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
