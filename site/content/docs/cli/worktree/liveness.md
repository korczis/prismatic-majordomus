+++
title = "majordomus worktree liveness"
description = "Whether anybody uses each path, a tree or a build directory: one `keep` or `reclaim` line per path with every reason found, and `blind` lines for the sources that could not be read. The one liveness predicate every deleting sweep asks; writes nothing. Exit 10 when a source could not be read (a sweep then removes nothing), 0 otherwise"
weight = 86
[extra]
route = "/docs/cli/worktree/liveness/"
command = "majordomus worktree liveness"
source = "apps/majordomus-cli/src/cli.rs"
+++
