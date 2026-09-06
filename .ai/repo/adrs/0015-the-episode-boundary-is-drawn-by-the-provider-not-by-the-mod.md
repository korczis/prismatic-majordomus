---
schema: adr/v1
id: adr-0015
kind: adr
title: The episode boundary is drawn by the provider, not by the model
status: proposed
date: 2026-09-06
tags:
  - architecture
  - provider
provenance:
  origin: extracted
  derived_from:
    - file:lib/capture.sh
    - file:lib/session_context.sh
    - decision:adr-0009
---

# 15. The episode boundary is drawn by the provider, not by the model

## Context

ADR 0009 settled where prompt capture belongs: below the model, in the provider's own hook,
because a worker cannot record what it never sees and an instruction in a bootstrap file is
a request rather than a mechanism. The session record has the same shape of problem and was
left with the other answer. `majordomus session start` and `session close` exist, they are
correct, and they run when a worker remembers to run them — which is never the episode that
mattered: the one that ended in a crash, in a compaction, or with somebody closing the
window. A store of episodes that went well and silence about the rest is worse than no store,
because it reads as a complete account.

Beside it sat an emptier defect. `.ai/local/session-contexts/` has been named by the layer
since the layout was written: `init` creates the directory, `.ai/README.md` calls it
"bounded working contexts", and nothing has ever written a file into it. No command, no
capability, no hook, no schema, no check, no test, no rule. A directory the skeleton creates
and nothing fills is a promise the layer does not keep, and a reader cannot tell an empty
store from an unimplemented one.

The two are one decision, because the second cannot be answered without the first. A working
context has to be written at a moment, and the only moment that is not a matter of somebody
remembering is the one the provider announces.

## Decision

Where a provider fires session lifecycle events, they draw the boundary. `capture install`
writes a shim for each and the matching entries in the provider's configuration; the shims
hand their payloads to `capture session --event start|end`, which opens and closes the
episode. Both are idempotent because the events are: a start fires again on a resume and a
compaction and keeps the open episode, an end fires with nothing open and writes nothing.
The event's reason decides `closed` against `interrupted`, and a reason the adapter does not
list closes the episode as interrupted, because calling a cut-short episode complete is the
worse of the two mistakes.

`session start` freezes the context the builder resolved into
`.ai/local/session-contexts/<stamp>--<session-id>.md`, and `session close` appends what the
close knows to the same document. That is what the store holds: not a narrative, and not a
second copy of the closed record, but the one fact neither neighbour has — what the worker
was actually told when the episode began. The closed record under `.ai/repo/sessions/` says
what the episode produced; the prompt archive holds the person's half of the exchange; this
holds the context that was given.

The store stays local, for two reasons that are not the same one. It names this machine —
the worktree path, the checkpoint that happened to be newest here — and a fact about a disk
is not a fact about the repository (ADR 0014). And it is a snapshot of a projection:
re-resolving it later produces a different document, so publishing it would publish
something no surface can reproduce.

Neither hook writes to standard output. This provider adds a `SessionStart` hook's output to
the model's context, and the local half of the layer is never loaded into a context
implicitly; diagnostics go to stderr, and the command never exits 2, exactly as
`capture prompt` never does.

The wiring is declared as `wired_by: provider-hook:<provider>:session` and proved by running
it, with one thing left out: the verifier drives a synthetic payload through the end shim
with the mutation disabled, because closing somebody's open episode is not a price a
diagnostic may charge. Everything else on the path — the shim resolving the repository,
finding the executable, the adapter matching, the payload parsing — runs as it does in a real
event.

## Alternatives rejected

*An instruction to the worker.* "Open a session when you begin" is the same request ADR 0009
rejected for prompts, and it fails in the same place: no behavioural test can prove it was
honoured, and the episodes it loses are exactly the ones nobody was in a position to close.

*Making the working context a shared object.* It would then be discovered, indexed and
projected like the closed record, which is the pattern this repository prefers. It cannot be
one: it names a path on one machine, and it is a snapshot no surface can regenerate.
Publishing an artifact nothing can reproduce is how a projection stops being a projection.

*Letting the closed record carry the working context's body.* The record is derived and
contract-shaped; the working context is a frozen projection plus a worker's notes. Merging
them would put an unverifiable body inside the one artifact whose value is that every field
of it can be checked.

*Deleting the directory instead.* Defensible, and it was the honest alternative to leaving it
empty. It was rejected because the fact it holds is real and nothing else holds it: after the
episode, nobody can reconstruct what the builder resolved at its open.

## Consequences

`capture install` now writes three shims and three configuration entries rather than one, and
a repository that installed the earlier version is told which entries are missing rather than
having its configuration rewritten. `capture status` reports two aspects per provider, and the
first column names the provider for the prompt aspect and `<provider>:session` for the other,
so a reader looking for one answer does not silently get the other.

`session start` and `session close` gain `--if-open` and `--if-none`. Both default to the
behaviour a person expects at a terminal — refusing — and the hook passes the tolerant value,
which is what keeps an event that fires twice from being a failure.

The store grows and nothing prunes it. A working context is not a restatement of state held
elsewhere: the closed record does not contain it and the builder cannot reproduce it, so no
cap governs it and `doctor` reports its size instead. That is the same policy the prompt
archive has, for the same reason.

A provider without lifecycle events is reported `unsupported` and stays that way. The boundary
this decision draws exists exactly where a provider announces it, and nowhere else; work done
outside a session remains attributed to no episode, as it was before.
