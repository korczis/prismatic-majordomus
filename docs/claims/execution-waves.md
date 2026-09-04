# Execution waves are computed from the graph, and issues in one wave that touch the same paths are reported as serialised

## What it means

A wave is the set of issues whose dependencies are all satisfied at the same depth. Wave zero is every root; an issue sits one layer past its deepest dependency. Waves are derived on every read and stored nowhere, so no execution plan can go stale. Sharing a wave is necessary for two issues to run concurrently and is not sufficient: if their declared `scope` paths overlap, the overlap is reported and they are serialised.

## How it works

The same Kahn layering that detects cycles assigns the waves. Scope conflict is a containment test on the declared paths of two issues in the same wave that are both `READY` or `ACTIVE`; a path equal to, inside, or containing another counts as an overlap.

## How to see it

```bash
majordomus plan waves
# Wave 1
#   I0002   READY     …
#   I0003   READY     …
#
# serialised by scope overlap:
#   I0002 shares lib/project.awk with I0003; they may not run concurrently
```

## What it does not cover

Scope is what an issue declares, not what it touches. Two issues that both edit a file neither of them claimed will conflict, and this reports nothing — the same limit `check` has.

## Why it exists

A false serialisation costs time. A false parallel costs a merge conflict and two workers discovering it after the work is done. The conservative direction is the cheap one.
