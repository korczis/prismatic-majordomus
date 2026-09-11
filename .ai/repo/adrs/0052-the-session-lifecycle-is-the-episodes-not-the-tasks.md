---
schema: adr/v1
id: adr-0052
kind: adr
title: The session lifecycle is the episode's, not the task's
status: proposed
date: 2026-09-11
tags:
  - sessions
  - continuity
  - governance
  - architecture
related:
  - rule:majordomus.lifecycle-observed
  - rule:majordomus.handover-integrity
  - rule:majordomus.session-lifecycle
  - rule:majordomus.checkpoint-freshness
  - file:.ai/repo/adrs/0014-a-closed-session-is-a-shared-object-of-the-layer-not-a-local.md
  - file:share/standard/majordomus/rules/lifecycle-observed.v1.md
  - file:share/events.yaml
  - file:docs/CONTINUITY.md
  - file:.ai/repo/policy.yaml
  - test:test/cases/130_lifecycle_survives_a_stale_task.sh
  - test:test/cases/131_freshness_has_an_age.sh
  - test:test/cases/132_health_sees_a_stopped_writer.sh
provenance:
  origin: authored
---

# 41. The session lifecycle is the episode's, not the task's

## Context

On 2026-09-11 a forensic audit of the continuity subsystem found that this repository had
written no checkpoint and no handover since 2026-09-05 — six days — while `doctor` and
`watch` reported health the whole time. Episodes opened and closed normally throughout;
`session.started` and `session.closed` kept arriving in the ledger. Only the two records a
future worker actually resumes from had stopped.

The cause is one line of policy expressed three times in code. Task
`t-20260905034523-a9f1` was marked `outcome: handed_over` on 2026-09-05 and never replaced.
From that moment:

- `lib/checkpoint.sh` refused every checkpoint, because a checkpoint was defined as *a
  progress note inside an active task*;
- `lib/capture.sh`'s compaction adapter skipped the derived checkpoint, because it first
  asked whether a task was active;
- `lib/capture.sh`'s end adapter skipped the derived continuation record for the same
  reason.

Each refusal was correct against its own contract, wrote to stderr, and returned 0 — which
is what a provider hook must do. None of them produced a record that the refusal had
happened, so nothing downstream could see the writer had stopped. Meanwhile the briefing
went on resolving the newest handover on the branch and quoting its `Next Action` verbatim
into every new episode, with an `advanced` label that says the recorded commit is an
ancestor of HEAD. `advanced` is a true statement about topology and says nothing about age.
Six days of workers were told to continue work that had been finished for six days.

Three separate defects share one premise: **that an episode is a thing that happens inside a
task.** It is not. A task is a unit of intended work that a person opens and closes. An
episode is a provider's conversation boundary — it begins when a client attaches and ends
when it detaches, whether or not anybody declared a task, and whether or not the last task
anybody declared was finished a week ago. Making the second depend on the first means that
the ordinary act of finishing a task silently disables continuity for every episode after
it, and the system is loudest about this exactly when nobody is looking.

A fourth defect follows from the same premise from the other side. Because the shell tool
owns the writes and `apps/majordomus-cli` owns the typed read model — `continuity.rs` opens
by saying so: *"It is read, never written. The lifecycle is the shell tool's"* — there is no
single place that could have noticed. The reader faithfully reported the newest record it
could find. The writer faithfully refused to write one. Neither is wrong; together they are
a system that cannot detect its own silence, because the invariant that would catch it
("lifecycle events are arriving, therefore derived state must advance") spans both halves
and is owned by neither.

## Decision

**An episode's lifecycle does not depend on task state.** A checkpoint and a handover are
artefacts of an *episode*. A task is an optional relation that an episode may carry and that
enriches the content of those artefacts. Task outcome may change what a continuation record
says; it may never decide whether one is written. Every guard of the form
`outcome != active -> skip` is removed from the lifecycle path.

**Every lifecycle event received is recorded before any guard runs.** Each provider adapter
appends a typed receipt to the ledger as its first act — `provider.session_start.received`,
`provider.pre_compact.received`, `provider.session_end.received` — and a typed failure event
if the work that follows does not complete. A hook may still return success to its provider,
because blocking the person's work is the worse failure; it may not leave the repository
without evidence that the event arrived. *Do not block the provider* and *pretend it
succeeded* are different instructions, and only the first is a contract.

**Freshness is a dimension of its own, beside divergence.** `exact | advanced | diverged |
different_context` is a statement about git topology and is kept unchanged. Beside it,
`fresh | aging | stale | unknown | invalid` is a statement about age, decided against
thresholds declared once in `.ai/repo/policy.yaml` under `session.freshness` and read by
every surface. A record may be `advanced` and `stale` at the same time, and that combination
is precisely the one that caused this outage.

**A stale record is history, never current.** The briefing may show a stale continuation
record — knowing what the last worker was doing is useful — but it is labelled as historical,
its age and the reason it is stale are stated, and its `Next Action` is *not* quoted as the
thing to do now. An unqualified instruction that was correct six days ago is worse than no
instruction, because the worker cannot tell it is wrong until after acting on it.

**Health measures writtenness, not only reachability.** `doctor` and `watch` gain invariants
of the form *if lifecycle events are arriving, the derived state they produce must advance*:
a checkpoint or handover older than policy while episodes keep opening is a finding, a count
of `session.started` that exceeds `session.closed` by more than the episodes actually open is
a finding, and a receipt with no resulting event is a finding. "The store is reachable" is not
a claim that anything is being written to it.

**The canonical direction is one domain service in the Rust executable.** The dual authority
described above — shell owns writes and lifecycle, Rust owns the typed read model, server,
API, MCP and Cockpit — is the structural reason this class of defect exists and is the thing
to remove. The target is a single canonical session/continuity domain service in
`apps/majordomus-cli`, owning the typed lifecycle state machine, identity, events, freshness,
persistence, recovery and projection; `.envrc`, the provider hook shims, the CLI, MCP, HTTP
and the Cockpit all become adapters and projections over it, and the shell keeps no domain
state machine, schema parser, continuity resolver, freshness engine or recovery engine. This
supersedes `continuity.rs`'s read-only contract. It does not introduce a second store: the
append-only ledger and the existing record directories remain the substrate, and their write
path moves under the service rather than being duplicated beside it.

That migration is staged, because the shell path is four thousand lines and is under active
change by other workers. This decision fixes the outage class in place and settles the
direction; the cutover is its own work, and `.ai/local/` stays local and gitignored across
all of it, with `.ai/repo/sessions/` remaining the only tracked carrier of durable
continuity.

## Consequences

A checkpoint written outside a task records `task: none` and is a normal record rather than
a refusal. `majordomus checkpoint` no longer fails when no task is open, which changes the
exit status of a command some scripts may call; the refusal it replaces was itself the
defect. Handovers written outside a task were already possible with `--no-task`, and the end
adapter now takes that path rather than writing nothing.

The ledger gains receipt events and therefore grows faster. `ledger.retention_max_lines`
already bounds it.

`session.freshness` thresholds are policy, so changing them is a repository decision with one
place to make it, and no surface may carry a second copy of the numbers.

The briefing gets longer by at most two lines when a record is stale, and shorter by the
length of a `Next Action` it no longer quotes — which is the point.
