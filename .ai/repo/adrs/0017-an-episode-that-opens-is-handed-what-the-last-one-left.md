---
schema: adr/v1
id: adr-0017
kind: adr
title: An episode that opens is handed what the last one left
status: proposed
date: 2026-09-06
tags:
  - architecture
  - continuity
  - provider
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0015
    - decision:adr-0009
    - decision:adr-0014
    - file:lib/derive.sh
    - file:apps/majordomus-cli/src/capability/builtin/continuity.rs
---

# 16. An episode that opens is handed what the last one left

## Context

ADR 0009 put prompt capture below the model. ADR 0015 put the episode boundary there too:
the provider fires an event when a sitting begins and another when it ends, and the boundary
is drawn from those or it is fiction. Both are settled and both are right.

Together they made *discovery* automatic and left *loading* exactly where it was. After
0015, a session opens without anybody typing a command, a working context is frozen, and a
closed record is written — and the next worker still learns what the last one knew only by
remembering to run `majordomus context`. That is the same failure the two earlier decisions
are against, moved one step later. `CONTINUITY.md` states the promise plainly: "the next
worker is given a briefing assembled from those files rather than a transcript to re-read."
Nothing gave it.

Two comparable implementations were studied before this was decided, both of which
Majordomus's own model already exceeds in every other respect.

`prismatic-platform` has a validated `.ai/` protocol — git-derived identity, tiered
resolution, divergence labels, atomic append-only publish — and beside it a second,
unvalidated store of several hundred hand-written session notes selected by modification
time. Its recovery is a four-line instruction in `CLAUDE.md`. Its own transfer document names
the missing piece: a start-event hook emitting the resolver's output is "the obvious upgrade
the design deliberately declines."

`hzs_radar` is where that protocol came from and is stricter still. Its rule is stated as
`HANDOVER != SOURCE OF TRUTH`, every recovered record is classified against git before it may
be trusted, and its `.ai/README.md` says outright: "Recovery is behavioral, not automatic
infrastructure … no startup hook injects every handover into context."

So all three designs stop at the same line, and each stops there on purpose. The reason given
is the same reason this repository gives, in the layer's own contract: nothing under `local/`
may be loaded into a model's context implicitly.

That clause is doing two jobs, and only one of them survives inspection. It forbids
publishing — a generator or a public surface carrying records that name one machine — and
that is unconditional and correct. It also forbids any automatic path from a record to a
worker, and that reading makes the continuation record write-only in the common case: it is
produced by a mechanism nobody has to remember and consumed by one everybody does.

Three concerns sit under the clause, and they are worth separating from it: a transcript
must never arrive; the context must not grow without bound; and a fact about a disk must not
become a fact about the repository. None of the three is about *whether* a record reaches a
worker. All three are about *what* reaches one, and *how much*.

## Decision

The start event writes a briefing to standard output, which the provider adds to the context
it is about to build. Two further events follow from the same argument.

**The briefing.** It carries the three facts a worker cannot get wrong quietly — which
episode it is in, what the last one left with the divergence label that says how far to trust
it, and what is blocking acceptance — plus the one section of a handover a resuming worker
acts on. It is bounded by `session.briefing_budget_lines`, and what it exceeds it names
rather than truncating in silence. It carries no conversation. Absence is printed rather than
omitted, because "no relevant handover" is a fact the next worker needs and silence is
indistinguishable from a briefing that failed to run.

It is the start event and only the start event. That event fires before there is a turn, and
its output furnishes one; the end and compaction events fire inside a turn already under way,
where output would alter what somebody is doing. This is the same distinction that forbids
the prompt hook from writing to standard output, applied consistently rather than reversed.

**The compaction event.** A compaction is not the end of an episode — the worker keeps going
— and it is the most common way state is lost while work is still in progress. The provider
announces it beforehand, which is the only moment at which anything can be written about a
context that is about to be discarded, so a derived checkpoint is recorded there.

**The continuation record at the end.** An episode that ends with its task still active now
writes a handover before the envelope closes. The two documents answer different questions:
the session record is an index of what the episode produced, and a handover is what the next
worker resumes from. Until now the next worker inherited the first and not the second.

**Derivation is what makes all three possible.** Every continuation record took its body on
stdin, which is right for a worker writing one deliberately and fatal when nothing is.
`--derive` composes the body from the task record, the ledger, git, the open questions and the
newest checkpoint — each already written, already validated, already identity-checked. A
derived body is a projection of records, which is precisely why a hook may write one: it can
be wrong only if a record it reads is wrong, and that record has its own gate. No model is
called and no network request is made.

**The layer's clause is narrowed rather than dropped.** `local/` is never published by a
generator, never served on a public surface, never read as policy. It reaches a model's
context by exactly two routes, both bounded by the policy and neither a transcript: the
context builder when a worker asks, and this briefing. `session.briefing_on_start: false`
restores the older silence for a repository that disagrees.

**Reading is separated from writing.** The Rust executable gains a `continuity` capability
that reads the local half and reports it over MCP, over HTTP and in the Cockpit, with the
same two-tier resolution and the same four divergence labels the shell tool uses. It never
writes: the lifecycle has one writer, and a second account of events the ledger already holds
is the thing this whole design refuses. It is served and never projected — no
`docs/generated/` artifact and no site page carries a record of the local half, and a test
proves it.

## Alternatives rejected

*Leaving loading behavioural, as both studied repositories do.* This is the status quo and it
is defensible: it keeps one clause simple and it cannot surprise anybody. It was rejected
because it makes the continuation record's value conditional on the one thing this design
says cannot be relied on — a worker remembering. `prismatic-platform`'s own notes call the
alternative obvious and decline it out of caution rather than on the merits.

*Injecting the whole `majordomus context` briefing.* It is already assembled, already
budgeted, and already the right content. It was rejected on size: that budget is 300 lines,
paid in every episode whether or not anything is needed. The briefing is a smaller document
with a smaller budget, and it ends by naming the command that prints the rest.

*Emitting only pointers — a path and a label, no content.* This would respect the clause
literally: nothing from `local/` would be quoted. It was rejected because the pointer without
the `Next Action` is an instruction to go and read a file, which is the remembering problem
with one extra step, and because a path plus a divergence label is already a fact about this
disk. Either the clause permits the route or it does not; a version that permits the metadata
and forbids the sentence draws no principled line.

*Writing the handover at the end from a model.* A summarised body would read better. It was
rejected because it puts a network call and a non-deterministic result on a path that runs
when a window closes, and because the tool makes no model calls anywhere else. An optional
model-written body remains possible on top of the derived one; the derived one stays
authoritative.

*Having the Rust executable write records too.* It already serves every other part of the
layer and could serve this one symmetrically. It was rejected because two writers for one
append-only store is two accounts of the same events, and the ledger's single-writer property
is what makes the session envelope derivable at all.

## Consequences

The layer's `local/` clause is amended, and the `majordomus.session-lifecycle` rule with it.
That rule's earlier text required silence on standard output from every event and a
behavioural case asserted it; both now state the narrower contract, and the case asserts the
briefing, its budget, and the silence of the other two events. A repository that vendored the
earlier package gets the amended rule through `rules vendor update`, which reports the change
rather than applying it.

`capture install` writes four shims and four configuration entries. A repository that
installed the earlier version is told which entry is missing rather than having its
configuration rewritten, exactly as 0015 arranged.

The briefing is paid for in every episode. Sixty lines is the declared price, it is visible
in the policy, and a repository that wants none of it turns the switch off.

`continuity.state` reads the local half on every call, which costs one git subprocess per
resolved record and is cached for two seconds — short, because the records are written by
another process and a longer cache would answer with an episode that had already closed.

The Cockpit gains a seventh area. It is the only page that shows the local half, and it is
the reason the Cockpit stays bound to the loopback interface.

Nothing here makes an episode depend on the briefing. Assembling it is best-effort: a
malformed record that could not be quoted is logged beside the working contexts and the
episode still opens, because an episode that opened and could not be described is worth more
than no episode at all.
