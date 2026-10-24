+++
title = "worktree.create"
description = "Make the worktree of a branch at its canonical path — the container beside the primary checkout, then the branch name with its hierarchy kept, derived and never given — or find it there. Idempotent: the canonical worktree already holding the branch is the answer (`existed: true`), with nothing created. Refused with the service's reason and nothing changed: the branch checked out somewhere else (`BranchAlreadyCheckedOut`, with the path it belongs at), the canonical path occupied by something else (`DestinationConflict`), a name git or the topology does not accept. A branch that does not exist here starts at `base` when given, else where the remote has it, else at the remote's trunk fetched now within a deadline, else the local trunk with how far behind it is (`start`). `git worktree add` runs under the worktree lock and a deadline, and is killed and refused when it passes it. The answer is the branch, the path, the `head` read back from the worktree, and whether it was created or existed; direnv's approval and the per-worktree hooks are carried to it as `worktree ensure` does, because this is that command."
weight = 213
slug = "worktree-create"
[extra]
id = "worktree.create"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
