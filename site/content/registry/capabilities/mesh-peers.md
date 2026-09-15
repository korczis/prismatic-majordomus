+++
title = "mesh.peers"
description = "Machine → runtime → session → claim: every runtime this one is linked to or has heard through the journal, grouped by machine, local first; each runtime with its liveness, the milliseconds since its beat rose and its link, each session with what it said and its claims. Remote and local runtimes share one shape, and the state digest closes the answer: equal digests, equal state."
weight = 67
slug = "mesh-peers"
[extra]
id = "mesh.peers"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
