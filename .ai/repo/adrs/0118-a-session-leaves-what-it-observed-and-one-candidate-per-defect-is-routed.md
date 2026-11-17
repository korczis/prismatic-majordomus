---
schema: adr/v1
id: adr-0118
kind: adr
title: A session leaves what it observed, and the boundary keeps one routed candidate per defect
status: proposed
date: 2026-10-09
tags:
  - knowledge
  - sessions
  - governance
provenance:
  origin: authored
  derived_from:
    - decision:adr-0010
    - decision:adr-0091
related:
  - file:.ai/repo/adrs/0091-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md
  - rule:project.never-store-transcripts
  - rule:project.no-new-nouns
  - rule:project.no-network-no-eval
  - file:lib/knowledge.sh
  - file:share/events.yaml
  - file:share/schemas/majordomus/knowledge/knowledge.v1.proto
  - file:.ai/repo/project/intents/a-session-leaves-what-it-observed.yaml
  - test:test/cases/980_a_session_records_what_it_observed.sh
  - test:test/cases/981_an_observation_becomes_a_grouped_candidate.sh
  - test:test/cases/982_a_candidate_is_routed_by_what_it_is_about.sh
---

# 118. A session leaves what it observed, and the boundary keeps one routed candidate per defect

## Context

ADR 0091 gave the repository a writer for what it knows: at every episode boundary, with no
model, the deriver turns four kinds of ledger line into knowledge candidates, and a person
promotes or rejects them. It did not give the repository a way to learn what is wrong with
the tool. On 2026-10-08 four concurrent sessions met friction with the tool — a fast-forward
that tripped the scope check, a stale rustdoc surface, a gate that passed while supporting
nothing, a number taken in a worktree nobody had pushed — and wrote thirty-six such
observations down, every one in an agent's private memory file or a message between sessions,
where the repository could not read it.

Two properties of the ADR 0091 pipeline stand in the way. Its evidence table is closed ("the
deriver implements exactly this list"), and none of its rows is an observation. And a
candidate says what class of statement it is, not what it is about or who should look at it,
so a candidate about a generated file and one about a project's own README wait in one
undifferentiated queue, and the same defect seen in three sessions is three unrelated files.

The owner asked that this happen without a prompt: sessions should improve the tool by being
used, not by being told to reflect.

## Decision

**An observation is a typed ledger line, `observation.recorded`, written by `majordomus
knowledge observe`.** It carries `kind` (`friction`, `workaround`, `defect`, `drift`,
`repetition`), `subject` (a repository path, or `capability:`, `rule:`, `command:`, `gate:`
followed by an identifier), a one-line `statement`, and zero or more typed `evidence`
references (`file:`, `test:`, `commit:`, `issue:`, `claim:`). It is an act, like `decision
add`: any provider runs the command, inside a task or outside one. The statement is refused
when it spans lines or carries a transcript marker, for the reason ADR 0091 refuses them in a
record. It is refused, with the command that opens one, when no episode is open: the deriver
selects a line by its episode stamp, and an observation without one would be written and never
derived.

**The deriver's table gains one row: `observation.recorded` yields a `lesson` whose
epistemics is `observed`.** The id, the file and the idempotence are ADR 0091's, unchanged:
the episode followed by a digest of the kind, the subject and the statement. One episode writes
its own file and no other, so two episodes, two worktrees or two machines never write one name,
nothing is read, merged and written back, and a merge of two branches that observed the same
thing adds two files and conflicts on none. `task.refused` and `task.gate` are not rows: a
refusal later satisfied and a gate that went red before it went green are the tool working, and
deriving a lesson from each would bury the observations under them. Which of them are friction
— the same gate red and green over the same inputs, a refusal never followed by a completion —
is a later decision, with its own measurement.

**A candidate derived from an observation carries what it is about and who should look at
it.** Two fields join knowledge/v1, both derived and both optional for every other record:
`about`, the observation's subject as written, and `route`, one of `platform`, `enforcement`,
`generator`, `documentation`, `project`, derived from the subject's structure and never from
its wording, in this order: a `command:` or `capability:` subject, a `rule:majordomus.*`
subject, or a path under the vendored rules directory or under the tool's own `share/` when
that directory is inside the repository, routes `platform`; any other `rule:` or a `gate:`
routes `enforcement`; a path the scope declares generated (`out.generated`) or the repository
marks `merge=derived` routes `generator`; a path under `docs/` routes `documentation`;
anything else routes `project`. In a repository that adopted the tool, `share/` is its own and
routes `project`, and only the tool's own identifiers and its vendored rules route `platform`:
a project's defect is never routed into the tool's governance.

**Recurrence is read, not written.** `knowledge candidates` groups the candidates by `about`,
each group listing its records and the episodes behind them, in the shell and the Rust reader
alike. A recurring defect is a group that grows; a rejected candidate stays rejected and its
group says so, and a later observation of the same subject is a new candidate in the same
group, shown beside the rejection rather than reopening it or silenced by it.

**Nothing new is the record of an ordinary session.** `knowledge.derived` with `written: 0`
is already appended at every boundary that evaluated an episode and found nothing (ADR 0091);
it is the NO_CHANGE the protocol asks for, and the case that proves derivation states it.

**Acting on a route is an act.** `knowledge promote` and `knowledge reject` are unchanged. The
route says which owner should look first; it opens no issue, writes no rule and sends nothing
anywhere.

This supersedes ADR 0091 in two sentences only: the evidence table gains the observation row,
and a candidate may carry `about` and `route`. Everything else it decided stands, its id
invariant included.

## Consequences

- Friction reaches the tracked tree at the next boundary of the episode that observed it.
- A defect that recurs is visible as one group with a growing list of episodes, while every
  file stays one episode's own.
- Candidates on a trunk that only accepts pull requests are as untracked as before; whatever
  I1984 decides for where candidates are written, grouping on read holds over it.
- Turning a routed candidate into an issue, a rule or an upstream proposal is not decided here.
  It is the next milestone, and it stays an act. So is deriving friction from `task.refused`
  and `task.gate`.

## Alternatives rejected

- **One file per (kind, subject), appended by every episode:** a read-merge-write on a tracked
  file from concurrent episodes loses updates in one checkout and conflicts on every merge
  across branches, its bytes depend on the order of derivation, and rejecting it would silence
  every later defect of that kind on that path.
- **A second store for observations** (an evolution database, a JSON catalogue): ADR 0010 and
  `project.no-new-nouns`. A candidate is already the reviewable unit.
- **Classifying by an LLM or by keywords:** ADR 0091 rejected reading the wording; a route from
  structure is reproducible and testable, and a model may still be used by whoever acts on a
  route.
- **Deriving from the transcript or the handover body:** `project.never-store-transcripts`.
