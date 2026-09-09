+++
title = "Worktree topology"
description = "Where every linked git worktree of this repository belongs and where each one is. The container is the primary checkout's sibling named with `-wt`, the path under it is the branch name with its hierarchy kept, and both are derived from git's own identity — the common directory, the registered worktrees, the branches — never from a registry, a configuration or the current directory. A worktree somewhere else is a typed diagnostic with a remedy; the migration that repairs it is a command-line operation of the same service."
weight = 19
slug = "worktree"
[extra]
id = "worktree"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
