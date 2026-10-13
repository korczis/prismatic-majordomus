---
schema: adr/v1
id: adr-0150
kind: adr
title: "Accumulation is watched at the episode's events: worktrees, merged branches and free disk are read against budgets the policy declares, said where a worker is listening, and reduced on the policy's consent"
status: proposed
date: 2026-10-13
tags:
  - architecture
  - operations
  - worktree
  - lifecycle
  - enforcement
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0052-the-session-lifecycle-is-the-episodes-not-the-tasks.md
  - file:.ai/repo/adrs/0055-disk-is-the-fifth-quantity-reclaimed-by-predicate-and-bounded-before-a-build.md
  - file:.ai/repo/rules/project/accumulation-is-measured.v2.md
  - file:apps/majordomus-cli/src/accumulation.rs
  - file:apps/majordomus-cli/src/capability/builtin/accumulation.rs
  - file:apps/majordomus-cli/src/commands/accumulation.rs
  - file:apps/majordomus-cli/src/commands/worktree.rs
  - file:apps/majordomus-cli/src/worktree/state.rs
  - file:lib/capture.sh
  - file:lib/derive.sh
  - file:test/cases/2986_accumulation_is_watched.sh
---

# 150. Accumulation is watched at the episode's events: worktrees, merged branches and free disk are read against budgets the policy declares, said where a worker is listening, and reduced on the policy's consent

## Context

A session's context is watched. The provider measures it, compacts before the window is
full, and fires an event; this tool records a checkpoint at that event, so what would be
lost is kept (ADR 0052). Nobody has to remember to look.

Three other quantities of a repository grow the same way and had no such watch:

- **worktrees** — one is registered by whoever starts a piece of work and removed by nobody;
- **merged branches** — a branch that has landed keeps its name;
- **free disk** — the build output under both, which is measurable as a problem only once
  nothing that could measure it still runs.

Each already has a command that measures it and one that reduces it safely
(`project.accumulation-is-measured`): `majordomus worktree`, `majordomus worktree cleanup
--remove`, `scripts/reap-orphans`. What was missing is the trigger. On 2026-10-13 this
repository had 247 registered worktrees, 118 local branches the trunk already reached and
47 GiB free on a 926 GiB volume; the volume had reached zero three times in the two days
before. Every time, a session noticed, ran the command by hand and wrote an alert on the
board by hand. `docs/WORKTREES.md` said it in one sentence: "nothing in the topology
reclaims anything and no budget is declared".

ADR 0055 bounded the demand side — a build is refused below a floor. That is the cliff. A
watch is what speaks before it.

## Decision

1. **The budgets are the policy's.** A block `accumulation:` in the policy declares
   `worktrees_max`, `merged_branches_max` and `disk_free_min_mb`. No reader carries a
   default: a quantity with no declared budget is measured and reported as `unbudgeted`,
   which is neither a pass nor a finding. A policy that predates the block watches nothing
   and removes nothing.

2. **One measurement, one capability.** `accumulation.status` reads the three quantities
   and judges each against its budget: `ok`, `over`, `unbudgeted`, or `unknown` with the
   reason when the reading could not be taken. A missing reading is never a zero and never
   a pass. The topology is read without a subprocess per worktree, so a start event can
   afford it. The capability is projected to the command line (`majordomus accumulation`,
   exit 10 when a budget is crossed), MCP, HTTP and the generated reference like any other.

3. **The watch reads at the provider's events**, because those are the moments a worker is
   already listening:

   | event | what the watch does |
   |---|---|
   | start | what is over budget goes into the briefing, before the first tool call |
   | prompt | the free disk alone, with one `df` and no executable; one line when it is under its floor, at most once per cooldown |
   | compact | a reduction, when the policy consents and one is due |
   | end | the same |

   No daemon is added and no interval is invented. A repository nobody is working in is not
   accumulating anything.

4. **Reducing is a consent the policy gives.** `accumulation.act` is `report` or `reduce`.
   `report` says what is over budget and names the command. `reduce` also runs it:
   `majordomus accumulation --reduce --if-due`, in the background, at most once per
   `cooldown_minutes` for the whole repository. The skeleton a new repository is written
   from declares `report`.

5. **A reduction removes only what is provably spare**, and proves it at the moment of
   removal. It is `majordomus worktree cleanup --remove --branches`, in process:
   - the worktree of a landed branch goes only when its branch has an upstream it is not
     ahead of, nothing has its working directory inside, and `git status` is empty — the
     readings `worktree cleanup --remove` has always taken;
   - the name of a landed branch goes only when nothing has it checked out, the trunk or the
     branch the trunk follows reaches its head, and it does not stand at either of those
     tips. It is deleted with `git update-ref -d <ref> <head>`, which removes the ref only
     while it still points where it was read.

   A branch at the trunk's own tip is where new work starts, and is kept. Every candidate
   that is kept is named with the reading that kept it.

6. **What the watch did is a record.** A reduction writes when it ran, which quantities
   asked for it, and what it removed, deleted and kept, under the primary checkout's local
   state, so every worktree of a repository reads one record and one cooldown. The next
   measurement carries it.

7. **A crossed budget is told to the mesh.** With `accumulation.announce_on_mesh: true` the
   start event makes an advisory claim of the watch's own session, `accumulation-watch`,
   whose intent names the machine and every quantity over its budget; the monitor of every
   linked machine shows it. When the budgets hold again the session is closed and the claim
   ends everywhere. No new channel is added: it is the mesh's own claim
   (`project.mesh-cooperation-is-authenticated`), and it replaces the alert sessions wrote
   on the board by hand.

8. **The watch can be switched off for a process tree.** `MAJORDOMUS_ACCUMULATION=off`
   silences every path. The test harness sets it, so no case run in this repository sweeps
   the worktrees of the machine it runs on; a dry run of an event never acts either.

## Consequences

- A worker hears "247 worktrees, budget 60" at the start of a session, not from a person.
- Deleting a branch name is now something a command does, on a separate flag
  (`--branches`). `worktree cleanup --remove` without it deletes none, as before.
- The briefing grows by at most four lines, and by none when every budget holds.
- Disk is reduced only through spare worktrees. Build output of branches that are pushed and
  not yet merged is not touched here; that is `scripts/reap-orphans` and pull request #921.
- A start event costs one more process when the watch is on.

## Alternatives refused

- **A timer in the shared server.** It would watch a repository nobody is in, and would put
  removal of worktrees into a server that deliberately writes nothing.
- **A default budget in the reader.** A fallback written beside a reader is a second source
  of truth for the same number (`lib/common.sh`, `mj_pol_req`).
- **An age.** "Older than a day" needs a time, and the sweep this repository still remembers
  made three active worktrees the oldest candidates by substituting an empty string for one.
  The tip of the trunk is a reading that cannot be missing.
- **Deleting names without a flag.** A branch is the only durable name a piece of work has.

## Status

Proposed. Implemented with rule `project.accumulation-is-watched`, case 2986 and issue
I2500. Accepting it is a person's act.
