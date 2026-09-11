---
id: project.the-session-context-is-derived
version: 1
kind: rule
title: A working context is derived, not stored
description: An episode's working context is composed on every read from the episode's record, git and its ledger window; the stored document is the opening snapshot alone and carries no heading a worker must hand-edit.
statement: "`majordomus session context` returns the episode's current state, composed at read time from its open record, from git now against git at the open, and from the ledger lines carrying its session id. The document under .ai/local/session-contexts/ is the opening snapshot and nothing else: it carries no section whose only producer is a worker remembering to edit it, and no command answers a request for a working context with a path unless a path was asked for."
status: active
class: blocking
depends_on: [project.no-new-nouns@1]
tags: [sessions, continuity, lifecycle]
---

# Rationale

`session start` wrote `.ai/local/session-contexts/<stamp>--<session-id>.md` once, and nothing
updated it for the rest of the episode. That document was called the working context and was
not one: it held the context builder's output from the moment *before* the worker had done
anything, which is the least informative instant in an episode to freeze. And
`majordomus session context` printed that file's **path**, so the command that exists to
answer "what is my working context" answered with a filename.

The document also carried a `## Notes` heading, inviting the worker to hand-edit its own
notes into it. Measured in this repository on 2026-09-11, across the 23 episodes this
checkout had opened: 23 carried the template, 0 carried a note. A lifecycle that depends on a
model remembering to edit a Markdown section is not a lifecycle, and a heading with no
producer is a database nobody writes to.

The obvious repair — a typed `session note` command to replace the heading — is refused by
the evidence, not only by `project.no-new-nouns`. This repository already has three commands
that write authored records: `decision`, `question` and `checkpoint`. Each has a declared
ledger event, and `mj_ledger_append` already stamps every line with the open episode's id.
Since episodes were introduced on 2026-09-06, across 23 episodes and six days, **zero**
decisions, questions, checkpoints or handovers have been recorded; the last one predates the
first episode. A fourth writer added to a layer whose three writers have no adoption produces
a fourth empty store. What those three lacked was a reader.

# Required behaviour

**The working context is composed at read time.** Its sources are the three that can be
checked: the episode's own open record (identity, when and by what it opened, the head it
opened at), git now against git at the open (branch, head, commits, files, whether the tree
is dirty), and the episode's ledger window (every line stamped with its session id). Nothing
caches it, so nothing can serve a stale one, and nothing has to remember to refresh it.

**The stored document is the opening snapshot and nothing else.** It stays frozen, because a
snapshot that changes is evidence of nothing, and it carries no section whose only producer
is a worker's memory. `--path` names it; the bare command does not.

**The builder is not re-resolved.** `majordomus context` resolves the repository's context and
is the only definition of that semantic (ADR 0004). The working context composes the
*episode* and refers to the builder for the rest.

**Authored records reach it through the nouns that exist.** Checkpoints, decisions and
questions are read back out of the episode's ledger window. A section with none says so and
names the command that would have filled it, so a worker reading its own context is told what
is missing rather than left to infer that the heading is decorative.

**Nothing is fabricated.** No model is called on this path. An empty section is reported as
empty.

# Failure behaviour

`test/cases/210_session_context_is_derived.sh` opens an episode, reads its context, then
commits and records a decision inside the episode and reads it again. It refuses a context
that did not change, one that is a bare path, one that still carries a `## Notes` template,
and one whose empty sections do not name the command that fills them.

# Verification

    majordomus session context          # composed, current
    majordomus session context --path   # the opening snapshot
    test/cases/210_session_context_is_derived.sh
