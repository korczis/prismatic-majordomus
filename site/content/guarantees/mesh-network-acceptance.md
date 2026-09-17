+++
title = "Separate Linux nodes on a real network discover each other by multicast, link, share claims and handovers, and recover from a partition and a crash, in a lab CI runs"
description = "Loopback tests prove the protocol; they cannot prove a network. The mesh lab starts four"
weight = 187
[extra]
claim_id = "mesh-network-acceptance"
status = "guaranteed"
source = "docs/claims/mesh-network-acceptance.md"
+++
{% raw %}

## What it means

Loopback tests prove the protocol; they cannot prove a network. The mesh lab starts four
Linux containers — three serving one repository, one serving another — each with its own
filesystem, node key, checkout and network interface on one bridge, with no seeds declared.
It asserts, through each node's own HTTP surface and command line, that the nodes find
each other by UDP multicast, link into a full mesh while the fourth is refused, replicate a
claim and a handover, converge on one digest, survive a node disconnected from the network
and a node killed and restarted, and verify with `majordomus mesh verify`. Each scenario's
verdict and timing is written to an evidence file.

## How it works

`test/mesh-lab/run` builds a Linux executable (or takes one), builds a node image around
it, creates the bridge, starts the nodes through `test/mesh-lab/node-entrypoint`, and runs
ten scenarios: `multicast_discovery`, `handshake`, `repository_isolation`,
`cross_node_claim`, `claim_conflict`, `cross_node_handover`, `three_node_convergence`,
`network_partition`, `process_crash_recovery`, `cli_verify`. Failures print every node's
cooperation status and log tail, write the evidence with the failing verdict, and exit 10.
The `mesh-lab` CI job runs it on every change to the crate or the lab.

## How to see it

```
test/mesh-lab/run                           # builds, runs, tears down (docker required)
jq . target/mesh-lab/evidence.json
MESH_LAB_KEEP=1 test/mesh-lab/run           # leaves the nodes up for inspection
```

## What it does not cover

One bridge is one layer-2 segment: the lab does not route between subnets, traverse NAT or
cross a WAN, and its nodes share one kernel. A physical multi-machine run is a separate,
manual procedure documented in docs/MESH.md.

## Why it exists

The mission is cooperation across machines. A claim of that needs machines — or the
closest reproducible thing to them — on a network that can be broken on purpose.
{% endraw %}
