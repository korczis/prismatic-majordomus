+++
title = "majordomus worktree create"
description = "Create the canonical worktree of a branch. A branch that does not exist here is fetched from the trunk's remote and taken up when the remote has it; otherwise it is created from --base, else from the remote's trunk fetched now (the local trunk only when the remote cannot be read, and then said, with how far behind). The path is derived; none may be given"
weight = 76
[extra]
route = "/docs/cli/worktree/create/"
command = "majordomus worktree create"
source = "apps/majordomus-cli/src/cli.rs"
+++
