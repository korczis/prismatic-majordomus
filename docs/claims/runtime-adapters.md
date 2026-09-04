# Opt-in runtime adapters will clamp read size, output size and fan-out from profile-derived limits

## What it means

**Planned for v0.2.** Optional, provider-specific hooks into a worker's runtime that clamp how many lines a single read may return, condense oversized tool output (head, diagnostic lines, tail, with a note that it was truncated), and cap how many sub-workers a session may spawn — with every limit derived from the task's profile rather than a global constant.

## How it works

Nothing is implemented. The design is drawn from a mechanism that ran briefly in the source environment: a read clamp, a semantic output condenser and a sub-worker budget, all real and all deterministic. It was rolled back within days because its limits were fixed constants that applied to every task alike, so the person disabled the governor and nothing was measured at all.

## How to see it

There is nothing to run. This page exists so the omission is visible rather than assumed.

## What it does not cover

Everything; it is a plan. When it lands it will be opt-in per provider and will never mutate a worker's input silently — drift will be surfaced before it is forced.

## Why it exists

Context per session is the largest cost term and the one profiles can only advise on today. Runtime clamps are the first place where profile axes become enforceable rather than projected.
