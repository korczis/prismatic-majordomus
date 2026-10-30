+++
title = "worktree.trunk"
description = "The checkout's `HEAD` and the local trunk branch, each counted against the remote-tracking trunk — commits behind and ahead, a merge in progress counting what it brings in — as of the last fetch, whose time is part of the answer; judged against the policy's `checkout.trunk_distance` as `current`, `lagging`, `stale`, `unjudged` (no threshold declared) or `unmeasured` (no remote trunk), with the commands that bring it current. What doctor renders and what `server.status` and the MCP instructions say about the tree a server serves (`project.a-checkout-serves-the-current-trunk`). Fetches nothing and writes nothing."
weight = 227
slug = "worktree-trunk"
[extra]
id = "worktree.trunk"
source = "apps/majordomus-cli/src/capability/builtin/worktree.rs"
+++
