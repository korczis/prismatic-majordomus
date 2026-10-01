# Separate Linux nodes on a real network discover each other by multicast, link, share claims and handovers, and recover from a partition and a crash, in a lab CI runs

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

The lab does not judge itself. `scripts/ci/mesh-lab-evidence` reads
`target/mesh-lab/evidence.json` afterwards and refuses it unless the record is a complete
acceptance: the schema it claims, an overall `passed`, all ten scenarios present with a
`pass` verdict and a detail that says what was measured, four nodes, both transports, and
an executable and a platform read out of a node. The CI job runs the reader as its own
step, so a lab that exits 0 having asserted less than it promises is a red job.
`test/cases/414_mesh_lab_evidence.sh` covers that reader — it feeds it a complete record
and nine incomplete ones, and holds its required set equal to the scenarios
`test/mesh-lab/run` reaches, so neither side can shrink without a failing case.

## How to see it

```
test/mesh-lab/run                           # builds, runs, tears down (docker required)
jq . target/mesh-lab/evidence.json
scripts/ci/mesh-lab-evidence                # judges that record; exit 10 if it is not an acceptance
test/run.sh 414_mesh_lab_evidence           # covers the reader, no docker needed
MESH_LAB_KEEP=1 test/mesh-lab/run           # leaves the nodes up for inspection
```

## What it does not cover

One bridge is one layer-2 segment: the lab does not route between subnets, traverse NAT or
cross a WAN, and its nodes share one kernel. A physical multi-machine run is a separate,
manual procedure documented in docs/MESH.md.

## Why it exists

The mission is cooperation across machines. A claim of that needs machines — or the
closest reproducible thing to them — on a network that can be broken on purpose.
