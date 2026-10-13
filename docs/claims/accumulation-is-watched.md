# Worktrees, merged branches and free disk are measured against budgets the policy declares at the provider's episode events, a crossed budget is said in the briefing, and nothing is removed without the policy's consent

## What it means

A repository's policy declares three budgets under `accumulation:` — how many worktrees,
how many local branches the trunk already reaches, and how much free disk. When a session
starts, the briefing names every quantity that is over its budget. When the free disk is
under its floor, the next prompt says so once. Nothing is removed unless the policy says
`act: reduce`; with it, the worktrees of landed branches and the names of landed branches
are removed in the background, and every candidate that is kept is named.

## How it works

`apps/majordomus-cli/src/accumulation.rs` reads the worktrees and branches from git without
a subprocess per worktree and the volume from `df`, and judges each against its budget:
`ok`, `over`, `unbudgeted` when the policy declares none, `unknown` with the reason when
the reading could not be taken. `accumulation.status` is that answer as a capability.

`lib/capture.sh` calls it from the provider's events: `mj_capture_accumulation_brief` at
the start event, `mj_capture_accumulation_prompt` at the prompt event (one `df`, no
executable), and `mj_capture_accumulation_reduce` at the compaction and end events. A
reduction is `majordomus accumulation --reduce --if-due`, which checks the policy's consent,
the crossed budget and the cooldown itself, then runs `worktree cleanup --remove --branches`.

## How to see it

```bash
majordomus accumulation
majordomus accumulation --format json | jq '.quantities[] | {id, measured, budget, state}'
bash test/run.sh 2986_accumulation_is_watched
```

The case builds a repository with a remote of its own, five worktrees and five merged
branches. It checks the readings and the exit code, the briefing with the watch on and off,
that `act: report` removes nothing, that `act: reduce` removes two worktrees and three names
and keeps unlanded work, a name at the trunk's tip and a branch that never reached a remote,
that a second reduction inside the cooldown does not run, that a compaction event starts a
reduction, and that the prompt event speaks once.

## What it does not cover

It does not remove build output of branches that are pushed and not merged, a worktree with
uncommitted work, or a remote branch. It does not refuse a build; the floor in
`lib/rust_bin.sh` does. It watches only while a provider fires events in the repository: a
machine filling up with no session open is not seen. A provider with no lifecycle adapter
gets no watch.

## Why it exists

Every accumulating quantity already had a command that measured it and one that reduced it,
and nothing ran either. On 2026-10-13 this repository had 247 worktrees and 118 merged
branches, and its volume had reached zero three times in two days; each time a session
noticed and ran the command by hand. ADR 0150 decides the watch; rule
`project.accumulation-is-watched` holds it.
