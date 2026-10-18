---
schema: adr/v1
id: adr-0177
kind: adr
title: A session proceeds through one entrypoint that composes the capabilities the repository already has
status: proposed
date: 2026-10-18
tags:
  - proceed
  - orchestration
  - planning
  - mesh
  - session
related:
  - file:.ai/repo/project/milestones/a-session-proceeds.yaml
  - file:.ai/repo/project/intents/a-session-proceeds.yaml
  - file:apps/majordomus-cli/src/plan.rs
  - file:apps/majordomus-cli/src/capability/builtin/plan.rs
  - file:apps/majordomus-cli/src/capability/builtin/brief.rs
  - file:apps/majordomus-cli/src/peers.rs
  - file:apps/majordomus-cli/src/mesh/state.rs
  - file:.ai/repo/workflows/task-lifecycle.md
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - file:.ai/repo/adrs/0144-an-intent-is-proposed-by-a-capability-and-its-stage-stays-derived.md
  - file:.ai/repo/adrs/0167-work-moves-through-seats-on-the-mesh-and-lands-in-combined-tops.md
provenance:
  origin: authored
---

# 177. A session proceeds through one entrypoint that composes the capabilities the repository already has

## Context

On 2026-10-18 the owner proposed `/proceed`: one word that makes any session, on any machine,
a productive worker of the repository — it resolves the context, reconciles who is working on
what, chooses the most valuable work that is safe to take, claims it, executes it, proves it
and hands it on, without a person re-explaining the project, choosing the issue, reading the
board or minding the worktree.

Every one of those steps already exists as a capability, and a session today performs the
composition by hand, from the bootstrap prose of CLAUDE.md and AGENTS.md:

| step | what exists |
|---|---|
| resolve | `brief.session`, `context`, `devcontext`, `intents.binding` |
| reconcile | `peers.list` (every checkout's board), `mesh.state`, `monitor.stream` |
| choose | `plan.next`, `plan.waves`, `devtask.issue`, `devtask.milestone` |
| claim | `mesh.claim` (exclusive), `peers.announce`, `worktree create`, `start` |
| execute | `executions.start`, the `implement` skill |
| verify | `check`, `obligations.closure`, `finish` |
| deliver | the landing and release seats (ADR 0167), `release.next` |

What is missing is not another orchestrator. It is:

- the composition itself, as one capability every surface projects;
- a choice that is aware of the board and the mesh: `plan.next` answers "the lowest-wave
  READY issue of the active milestone, highest priority first, then id", and knows nothing of
  a live claim on that issue, of a scope another worker holds, or of how much other work the
  issue unblocks — the 10-17 sessions chose by grepping the board by hand;
- an explanation a reader can check: why this issue, why not the next one, why not by
  another worker, what it unblocks, what evidence finishes it;
- idempotence: a second `/proceed` in a session with live work must return that work, not
  open a second claim, worktree or task.

## Decision

1. **One capability, `proceed.plan` (a query), decides.** It reads the repository's board,
   the live mesh claims and the plan graph, and answers one decision: the selected issue, the
   hard constraint every passed-over READY issue failed and the hold that failed it, the terms
   of the order, what the selection unblocks, the evidence that finishes it, and the commands
   that take it. Its constraints are `claimed` (a mesh claim names the issue or claims its
   record), `named` (a hold's words name its id, quoted, because the board is where work is
   still taken) and `scope_held` (an exclusive mesh claim meets its scope); an issue that is
   not READY is never a candidate; an unbound or uncritiqued issue is refused by the acting
   command's `start --issue`. A decision that could not read the board or a declared mesh is
   `provisional` and gives no commands. It writes nothing; `majordomus proceed plan`, the MCP tool `majordomus_proceed_plan`
   and `GET /api/v1/proceed/plan` are its projections (ADR 0004), declared once with
   `capability!`.
2. **The ranking is explainable before it is clever.** Eligibility is a hard filter, and it
   removes an issue from the plan's own hand-out order (`plan.next`'s: the asked milestone,
   then wave, priority and id) without reordering what remains; the number of distinct issues
   each candidate transitively unblocks (counted once, never per path) is shown beside it.
   Ordering by that number is the second step, taken in `plan.rs` and `lib/project.sh`
   together (case 99 holds the two engines to one answer), so that `plan.next` and
   `proceed.plan` keep one order. A weighted score
   (impact, confidence, effort, risk, coordination cost) is a later change that must be
   calibrated against finished work and must show its terms; it is not decided here.
3. **One command, `proceed`, acts.** It takes the decision and performs, through the existing
   capabilities only, the exclusive mesh claim, the board announcement, `worktree create` and
   `start` bound to the issue. Run again in a session that holds live work, it returns that
   work and creates nothing.
4. **The `/proceed` skill is generated and thin.** `.ai/repo/skills/proceed/` names the
   command and the existing `implement` skill and holds no rule list, workflow copy or issue
   list; a change to the capabilities never requires editing it.
5. **It never lands and never releases.** Delivery stays with the one landing seat and the one
   release seat (ADR 0167): `proceed` ends a finished task by handing its head and receipt to
   the landing seat on the board and the mesh.
6. **Exclusivity across machines is the mesh's.** Until `mesh-journal-converges` gives claims
   fencing, two machines can admit the same claim; `proceed` reports such a conflict as the
   fold names its winner and stops the loser. It adds no lock of its own.

## Consequences

- A fresh session's first act can be `/proceed` instead of the bootstrap prose; the prose
  shrinks to "run `majordomus proceed`" once the milestone is done.
- The choice moves from a person reading the board to a derivation, and becomes testable:
  adding a dependency to the graph changes the ranking and the reason, with no list edited.
- The 6 acceptance scenarios of the proposal (fresh session, concurrent sessions, worker
  failure, impact selection, delivery, idempotence) become cases of the milestone.
- Not decided here: event-driven proceeding without a person's word (a watchdog that calls
  the same capability), a portfolio optimiser across several workers, and automatic release
  on every landing; each is a later ADR once this one is measured.
