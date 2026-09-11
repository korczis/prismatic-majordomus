---
schema: adr/v1
id: adr-0046
kind: adr
title: A working context is derived and a closed record is composed; neither is a document somebody must remember to write
status: accepted
date: 2026-09-11
tags:
  - sessions
  - lifecycle
  - continuity
  - governance
related:
  - rule:project.episode-record-carries-its-episode
  - rule:project.the-session-context-is-derived
  - rule:project.no-new-nouns
  - file:lib/session_context.sh
  - file:lib/derive.sh
  - file:.ai/repo/sessions/README.md
provenance:
  origin: authored
---

## Context

An execution episode has two artefacts. While it runs it has a *working context* under
`.ai/local/session-contexts/`; when it closes it has a *record* under `.ai/repo/sessions/`.
Both were defined, validated, counted healthy, and empty of the one thing a reader wants.

Three measurements, taken in this repository on 2026-09-11:

**The working context was frozen at open.** `session start` wrote the document once — front
matter, then the context builder's output verbatim — and nothing updated it afterwards. A
document written before the worker has done anything is a snapshot of what it was *told*,
which is the least useful moment to freeze and call a working context. Worse,
`majordomus session context` answered with that file's **path**, not its content, so the
command that exists to answer "what is my working context" returned a filename.

**`## Notes` was a heading with no producer.** Every document carried a `## Notes` template
inviting the worker to hand-edit its own notes in. Across the 23 episodes this checkout had
opened, 23 carried the template and 0 carried a note.

**The closed record's body was empty by construction.** The `session/v1` contract made every
machine-derivable field mandatory — `head`, `branch`, `commits`, `changed_files`, the
reference lists — and left the one field carrying meaning optional. Its only producer was a
person typing into `majordomus session close`, and the automatic lifecycle closes an episode
with `mj_session_close < /dev/null`. So on the automatic path, which is every path, the body
was blank. A record of an episode that had made ninety-four commits said nothing about any
of them.

A fourth measurement explains the second and is the one that changed the decision. The
repository already has three commands for authored records — `majordomus decision`,
`majordomus question`, `majordomus checkpoint` — each with a declared ledger event, and
`mj_ledger_append` already stamps every line it writes with the open episode's id. Since
episodes were introduced on 2026-09-06, across 23 episodes and six days, **the number of
decisions, questions, checkpoints and handovers recorded is zero**. The last one predates
the first episode.

That rules out the obvious fix. The `## Notes` section is not empty because Markdown is the
wrong interface; a typed `session note` command would have had exactly the adoption its
three older siblings have, which is none. The interface shape was never the bottleneck.

## Decision

**The working context is derived, not stored.** The document under
`.ai/local/session-contexts/` keeps exactly one job — it is the *opening snapshot*, and a
snapshot is legitimately frozen, because a snapshot that changes is evidence of nothing.
The working context becomes something else: not a file, but a composition performed on every
read, from three sources that can each be checked — the episode's own open record, git now
against git at the open, and the episode's ledger window. `majordomus session context`
returns that composition; `--path` returns the snapshot, which is what it returned before.

Nothing caches it. The shape the problem invites is *events + state -> context -> cache*, and
the cache is deliberately left out: at this size the projection costs one awk pass over a
ledger window, and a cache would add only a way to be wrong. A derived context cannot be
stale, and nothing has to remember to refresh it, which is the property the frozen document
lacked.

It does not re-resolve the context builder. `majordomus context` is that capability, and a
second definition of one semantic is a design defect here (ADR 0004). What `session context`
composes is the *episode*; what the builder resolves is the *repository*.

**Authored notes get no new noun.** `decision`, `question` and `checkpoint` already exist,
already emit events, and are already episode-attributed by the ledger's session stamp. What
they lacked was a **reader**. The `## Notes` heading is removed, and the composed context and
the closed record's body both read those three back, scoped to the episode. This respects
`project.no-new-nouns`, and it is also the honest reading of the evidence: adding a fourth
writer to a layer whose three writers have zero adoption would have produced a fourth empty
store, not a lifecycle.

**A closed record's body is composed and never empty.** An authored summary on standard
input is still taken where one is given. Where none arrives, the body is composed from what
the episode wrote down (its checkpoints, decisions and questions, scoped by its own session
stamp) and from what git and the ledger prove (the commits with their subjects, the files,
the plan issues, how it ended). The sections are declared in `policy.yaml` under
`session.record_sections`, with the same contract the handover sections have: a section named
there with no writer is a configuration error reported by name, never a silently empty
heading.

**Nothing fabricates a summary.** Where the episode recorded nothing authored, the section
says so by name and names the command that would have recorded something. No model is called
anywhere on this path. An acknowledged gap is a fact a reader can act on; an invented
narrative in an append-only record is one nobody can tell from a real one afterwards, which
is strictly worse than the blank it replaces.

## Consequences

A worker reading its own working context mid-episode is now told, in the document itself,
which of its records are empty and which command fills each one. That is the only pressure
toward adoption this decision creates, and it is deliberately advisory: nothing refuses to
close an episode over missing prose, because an episode cut short is exactly the case that
most needs a record written.

The empty-authored-layer finding is **not** closed by this decision. Its root cause is
visible but out of this change's scope: every writer in that layer — `decision`, `question`,
`checkpoint` — is command-line only, while every worker since 2026-09-06 has been an agent
reaching this repository over MCP, and the capability registry is read-only by contract
(`plan.rs` asserts `every_capability_is_a_query`, and state transitions live on the command
line by design). An agent therefore cannot record a decision through the surface it actually
uses. Changing that means deciding whether the registry may carry writes, which is a
separate decision and needs its own ADR; this one records the finding and does not pre-empt
it.

The two documents now answer disjoint questions and can no longer disagree, because only one
of them is stored.
