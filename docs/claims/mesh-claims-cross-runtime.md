# An exclusive claim made on one runtime excludes an overlapping claim on every linked runtime, and concurrent claims resolve to one named winner everywhere

## What it means

A session on machine A claims `apps/majordomus-cli` exclusively. A session on machine B
that then claims `apps` exclusively is refused with `claim_conflict`, and the refusal names
A's claim. The same holds between two sessions of one server, two worktrees of one
machine, or three runtimes linked only through the middle one: an exclusive claim excludes
everywhere it is known, by the same rule. When two sides of a network partition each claim
the same scope — each was right to, since neither could see the other — healing the
partition does not leave two holders: every runtime folds the same events and names the
same winner, the claim with the lowest `(lamport, stream, seq)`; the other is listed as
`conflicted` with the winner's key.

## How it works

Claims are events of the replicated journal (`apps/majordomus-cli/src/mesh/journal.rs`),
signed by the claiming runtime. `mesh::state::fold` is a pure function over the set of
events and each stream's liveness, applying events in `(lamport, stream, seq)` order, so
arrival order and duplicate delivery cannot change the result — property tests hold both.
`admission_conflicts` refuses a new exclusive claim that meets a live exclusive claim of
another session; paths meet by `peers::claims_meet`, the peer board's own predicate. A
claim lives while it is unreleased, its session is open, and its runtime's beat keeps
rising.

## How to see it

```
majordomus mesh claim apps/majordomus-cli --session s1 --issue '#184'
majordomus mesh state                      # every claim with held/conflicted/expired
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh::state
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_cooperation
```

`exclusivity_holds_for_any_interleaving` and `fold_is_order_and_duplicate_independent` are
the properties; `a_partition_expires_the_far_side_and_healing_reconciles_to_one_winner`
and `test/mesh-lab/run` (scenario `network_partition`) break a real link and heal it.

## What it does not cover

It is not a lock service with strong consistency: during a partition both sides may hold
the same scope, and the conflict is named after healing rather than prevented. An advisory
claim (the peer board's announcements) reports overlaps and refuses nothing.

## Why it exists

Several AI sessions on several machines working one repository collide on files unless a
claim made anywhere is honoured everywhere — and a promise that held only between sessions
of one server would fail exactly when the other worker runs somewhere else.
