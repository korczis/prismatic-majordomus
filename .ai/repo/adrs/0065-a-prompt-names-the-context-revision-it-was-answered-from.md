---
schema: adr/v1
id: adr-0065
kind: adr
title: A prompt names the context revision it was answered from, and one domain writes both
status: proposed
date: 2026-09-15
tags:
  - sessions
  - continuity
  - context
  - provenance
  - architecture
related:
  - file:.ai/repo/adrs/0009-prompt-capture-happens-below-the-model.md
  - file:.ai/repo/adrs/0015-the-episode-boundary-is-drawn-by-the-provider-not-by-the-mod.md
  - file:.ai/repo/adrs/0017-an-episode-that-opens-is-handed-what-the-last-one-left.md
  - file:.ai/repo/adrs/0047-the-session-domain-is-typed-and-its-identities-are-not-interchangeable.md
  - file:.ai/repo/adrs/0052-the-session-lifecycle-is-the-episodes-not-the-tasks.md
  - file:docs/RUNTIME_CONTINUITY.md
  - file:.ai/repo/project/milestones/runtime-continuity.yaml
---

# 65. A prompt names the context revision it was answered from, and one domain writes both

## Context

An audit on 2026-09-15 (`docs/RUNTIME_CONTINUITY.md`) measured why nothing in this repository
can say what context a worker had when it answered a prompt:

- Three compilers produce context. The SessionStart briefing (`lib/derive.sh`) is what a worker
  receives and is never stored or fingerprinted. The frozen working context
  (`lib/session_context.sh`) stores a different builder's output (`lib/context.sh`). The only
  compiler with a fingerprint, `devcontext::compile`, has no consumer.
- Freshness compares git head and branch only. A changed rule, ADR, issue or handover never
  invalidates anything, and nothing re-delivers context after a session starts.
- A prompt record is written at UserPromptSubmit with a closed field set: no context, task,
  issue, model or result. No hook records how a turn ended.
- The shell writes every session record, Rust reads them and holds writers nothing calls, and
  the invariants that span both — an event received advances derived state; the context
  delivered is the context recorded — have no owner. ADR 0047 deferred moving the writes into
  Rust because four workers were editing the shell path at once.

The reference repository named for comparison was measured weaker on each of these, so the
design is this repository's own.

## Decision

The repository owner decided three things on 2026-09-15, and this record states them and what
follows from them.

**One domain writes.** The Rust session domain (`apps/majordomus-cli/src/session/`, with new
`context/`, `prompt/` and `hook/` modules) is the single writer of episodes, the ledger, context
revisions and prompt records. Shell functions delegate to it. ADR 0047's deferral is ended: its
reason, concurrent edits to the shell path, is a coordination problem the peer board and the
milestone's slice order now answer, and the split authority it preserved is itself a measured
root cause.

**A context is a revision.** `devcontext::compile` is the only compiler. Each compilation is a
content-addressed object and a numbered, immutable revision under `.ai/local/context/`,
carrying task, issue and milestone identity with provenance and a manifest of every dependency
the compiler read. The briefing and `majordomus context` are renderings of a revision. A
revision is never rewritten; a changed dependency produces the next one.

**Provenance is recorded at the provider boundary.** Majordomus still never invokes a model, and
a gate holds that. The boundary it owns is the provider's hook and the MCP session: the
UserPromptSubmit hook ensures the current revision within a measured budget, delivers it or a
typed change notice through the provider's hook output, and writes a prompt record naming that
revision; a Stop hook writes the terminal state. A client whose provider exposes no such event
is reported as unobservable, never approximated.

Two earlier contracts change, and are named here rather than drifting:

- ADR 0017's statement that the prompt hook writes nothing to standard output. It now writes the
  provider's hook output when the revision changed, and nothing otherwise.
- ADR 0047's refusal to model a session write as a registry command. `context.refresh` is a
  command that writes `.ai/local` only, exposed to the checkout's own loopback server.

## Alternatives rejected

**A model runtime inside Majordomus**, wrapping provider calls so every prompt passes one
function. It changes what the product is, contradicts its public claim, and a hook-based client
would still bypass it.

**Keeping the shell as the writer** and adding revisions there. Fastest, and it keeps the split
authority the audit found to be a cause.

**Reviving the session branches of 2026-09-11.** They carry sound ideas (ADRs 0043 and 0046) and
sit about five hundred commits behind master in the most conflict-prone subsystem; they are
mined, not merged.

**A time-to-live on context.** A revision is stale because something it read changed, and the
manifest names what; a clock names nothing.

## Consequences

- Rust becomes mandatory for writes. An installation without the executable loses capture and
  ledger writes and must see a named refusal on the command line and a visible degraded notice
  in hook output, never silence.
- A cold refresh needs a full index build, which costs seconds on this repository. Delivering a
  fresh revision within one prompt is not guaranteed; the last revision is kept and marked
  degraded, with backoff, and the worker is told.
- Claude Code's hooks carry no model, no failure signal and no causal parent. Most prompt records
  will say the model was unobserved, `failed` is claimed only if the pinned provider documents a
  failure event, and a parent prompt is known only as the first prompt of its provider session.
- Existing prompt records are not rewritten. A classified index states for each field whether it
  is verified, derived, inferred or unresolved.
- The work lands as the slices of milestone `runtime-continuity` (I1700–I1703), each merged on
  its own.
