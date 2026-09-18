# Runtimes linked only through a relay converge on one state digest, each event held once, with no replication loop

## What it means

A is linked to B and B to C; A and C never talk. A claim made on A is held on C, a claim
made on C refuses an overlapping claim on A, and all three runtimes report the same state
digest. Every event is stored once on every runtime — the counts are equal — and once the
state has converged, further heartbeats carry no events at all: replication stops instead
of bouncing A → B → A.

## How it works

Replication compares high-water marks: a sync round sends a peer exactly the events its
marks say it lacks, per stream, so an event reaches each runtime once per link and has
nowhere further to go. Events keep their origin's signature through every relay, so C
verifies A's key, not B's word, and a relay can neither forge nor alter what it forwards.
Duplicate deliveries — possible when a runtime has two paths — are recognised by
`(stream, seq)` and change nothing. The fold is order-independent, so the digest depends on
the set of events and the liveness verdicts alone
(`apps/majordomus-cli/src/mesh/journal.rs`, `apps/majordomus-cli/src/mesh/state.rs`).

## How to see it

```
curl http://127.0.0.1:8741/api/v1/mesh/state | jq '.state.digest'   # on each runtime
curl 'http://127.0.0.1:8741/api/v1/mesh/events?limit=1000' | jq '.count'
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation three_runtimes
```

`three_runtimes_in_a_line_converge_through_the_middle_without_loops` runs three server
processes in a line; `a_relay_carries_events_it_cannot_forge` and
`any_permutation_with_duplicates_converges` are the unit and property proofs;
`test/mesh-lab/run` (scenario `three_node_convergence`) repeats it across three Linux nodes.

## What it does not cover

Delivery is at-least-once with idempotent application, not exactly-once. Convergence is
eventual: during a partition each side converges on what it can reach.

## Why it exists

A mesh proven with two nodes has not been proven: relays, duplicates and loops only appear
with a third.
