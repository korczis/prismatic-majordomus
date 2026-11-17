---
id: majordomus.a-shared-plan-lives-in-the-repository
version: 1
kind: rule
title: A plan that more than one session works from lives in the repository
description: A plan, brief or ledger that several agents or sessions work from is committed to the repository; a session's scratch space holds only that session's transient files.
statement: Commit to the repository every plan, brief or ledger that more than one agent or session works from, keep what is derived in it derivable from its sources, and use a session's scratch space only for that session's transient files.
status: active
class: advisory
depends_on: [majordomus.externalise-decisions@1, majordomus.handovers-carry-state@1]
tags: [continuity, coordination, records]

x-majordomus:
  reviewed_because: whether a file is a plan that more than one session works from, and where each session keeps its scratch space, is read from how the work is shared, not from a shape a check could match; a reviewer decides it, and a repository may add a project rule with a validator, such as one that no handover or issue cites a path under a temporary directory
---

# Rationale

A session's scratch space (a temporary directory, a scratchpad, a cache under the session's
own identity) belongs to that session. Other sessions cannot read it, nothing reviews it,
nothing versions it, and it is cleaned up on the session's schedule, not the project's. A
plan kept there is a plan only one worker can see and nobody can recover. Every other
worker on the project then works from a copy, a summary or a memory of it, and those drift.

Observed in algoglyph, a public repository: the multi-slice plan that several agents worked
from lived in one session's temporary scratchpad. Peer sessions could not read it at all.
It was deleted there in the middle of the project. It was rebuilt from memory, and the
solver designs it held were lost, because no copy of them existed anywhere else.

# Required behaviour

- **Shared means committed.** A plan, brief, ledger, slice list, design note or checklist
  that more than one agent or session reads or updates is committed to the repository, on a
  branch every worker can fetch, before the second worker starts from it. Pushing it is part
  of sharing it: a commit no remote holds is still one machine's file.
- **Derived stays derivable.** What the plan computes from other records (a status table, a
  count, a list of open slices) is generated from those records by the repository's own
  derivation, and the plan says which parts are generated. Only the decisions, designs and
  reasons are written by hand.
- **Scratch is for one session.** A session's scratch space holds files that session alone
  creates, uses and discards: intermediate output, a downloaded archive, a probe script.
  Anything another worker will need, or that the same work needs after the session ends,
  moves into the repository first.
- **A brief is kept as received.** A brief or order that the work answers is committed with
  its source, so a later session reads the brief and not a recollection of it.

Moving a plan into the repository is not a reason to lose its history: the first commit
carries it as it stood, and later commits change it.

# Failure behaviour

Nothing decides this rule directly: it has no validator, so a violation is reported by
nobody. It is normative text. A worker applies it before handing work to another session or
fanning it out, and a reviewer treats a handover, brief or task that points into a session's
scratch space as pointing at nothing.

# Verification

Every plan, brief or ledger named by a handover, an issue, a peer announcement or a task
that more than one session works on resolves to a path in the repository at a pushed
commit. No such reference points into a temporary or session-scoped directory. A repository
may add a project rule with a validator for the part a machine can decide, for example that
no handover or issue cites a path under a temporary directory.
