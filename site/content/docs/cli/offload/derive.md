+++
title = "majordomus offload derive"
description = "Derive a worktree on a declared host and stage what it changed; nothing is committed or pushed. Exit 2 on an untracked file or an unresolved merge, 10 when the derive fails, 11 when it deletes a tracked file, 12 when no host can take it, 75 when the queue's wait runs out"
weight = 97
[extra]
route = "/docs/cli/offload/derive/"
command = "majordomus offload derive"
source = "apps/majordomus-cli/src/cli.rs"
+++
