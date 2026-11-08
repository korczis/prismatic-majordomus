+++
title = "proceed.plan"
description = "The READY issues in the plan's hand-out order (the asked milestone first, then wave, priority and id, as `plan.next`), each refused when another worker holds it: a live mesh claim names it as its issue or a hold claims its record (`claimed`), a board or mesh announcement names its id as a word of its own (`named`, quoted beside it), a live exclusive mesh claim of another session meets its scope (`scope_held`), or `plan.delivery` places it at stage `branch` or `pr_open` on a branch or an open pull request that is not the caller's (`in_flight`, naming the pull request and its head or the branch and its commit, whether or not its author still holds a claim); advisory holds over its scope are listed as `shared_with` and refuse nothing. The first issue nothing refuses is `selected`, with what it unblocks, what finishes it and the commands that take it; `reason` says why in one sentence, and `sources` whether the plan, the board, the mesh and the delivery could be read. A decision that could not read the board, a declared mesh, or the branches and the forge the delivery reads, is `provisional` and gives no commands. The caller's own session (its mesh claims on this runtime, its board announcements, and the branch checked out where the decision is composed or on its board peer's checkout) is never held against it; a session of the same name on another runtime is another worker. Composed by the server holding this checkout's lease, which holds the board and the mesh; the command line asks it. Writes nothing."
weight = 177
slug = "proceed-plan"
[extra]
id = "proceed.plan"
source = "apps/majordomus-cli/src/capability/builtin/proceed.rs"
+++
