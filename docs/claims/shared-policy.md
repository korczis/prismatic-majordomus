# One policy will be shared across repositories and workers

## What it means

**Planned for v1.0.** One canonical policy distributed to many repositories, with the same projections, the same finish contract and the same drift checks everywhere a team's workers run.

## How it works

Nothing is implemented. v0.1 is local-first: one policy per repository, one active task per checkout, overlap seen across worktrees on one machine.

## How to see it

There is nothing to run.

## What it does not cover

Everything about organisations, today. There is no server, no account and no central store, and there will be none in the local tool.

## Why it exists

The design's expansion path — shared policy across a team, then measurement, then recommendations — is stated so that the local tool is built with it in mind and not against it.
