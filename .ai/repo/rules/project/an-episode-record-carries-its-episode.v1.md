---
id: project.episode-record-carries-its-episode
version: 1
kind: rule
title: An episode record carries its episode
description: A closed episode's record has a body composed at close from what the episode wrote down and what git and the ledger prove, and the lifecycle that writes it commits it.
statement: "`session close` writes a record whose body is never empty: an authored summary when one was supplied, and otherwise one composed from this episode's own checkpoints and decisions and from what git and the ledger can prove about it. The lifecycle that writes the record also commits it, so that the record reaches the surfaces that read the tracked tree."
status: active
class: blocking
depends_on: [project.diagnostics-decide-the-exit@1]
tags: [sessions, records, lifecycle]
---

# Rationale

The `session/v1` contract made every machine-derivable field mandatory — `head`, `branch`,
`commits`, `changed_files`, the reference lists — and left the one field carrying meaning
optional. That field is the body: what the episode was for, and how it turned out.

Its only producer was a person typing into `majordomus session close`. The lifecycle closes
an episode from the provider's end event, and `lib/capture.sh` closes it with
`mj_session_close ... < /dev/null`. So on the automatic path — which is every path — the
body was empty *by construction*, not by omission. On 2026-09-11 a record of an episode
that had made ninety-four commits read, in full:

    ---
    schema: session/v1
    ...
    commits: [ "a3c7102", "5314bd6", ... 94 entries ... ]
    tasks: []
    issues: []
    ...
    ---

and nothing else. That is a receipt, not a record. Every field the validator checked was
present, so the section reported itself healthy — the validator measured the half of the
contract that had a producer and did not measure the half that did not.

The second half of the same defect: nothing committed the record. Every surface that reads
session records — the site's `/sessions/` pages, the Rust index, the registry, `session
list` in another clone — reads the **tracked** tree. A record written into the working tree
and left there is written, validated, counted healthy, and invisible everywhere a reader
would actually look. Five records sat untracked in the primary checkout while `doctor`
reported the section green, and `lib/session.sh` said so in its own comment: *"The lifecycle
writes a record when an episode closes and nothing commits it, so the gap is the default
rather than an accident."*

A gap a comment describes and no check refuses is a gap the repository has decided to keep.

# Required behaviour

**The body is composed, never blank.** `session close` reads an authored summary from
standard input as before; where none arrives it composes one from two sources, and says
which is which:

- what the worker itself wrote down while the episode ran — its checkpoints, the decisions
  it recorded, the questions it opened. This is the only material in the repository that
  carries intent.
- what git and the ledger prove — the commits with their subjects, the files, the plan
  issues moved, how the episode ended.

Where the worker recorded nothing, the section **says so by name** and names the command
that would have recorded something. It does not invent a summary. A fabricated narrative in
an append-only record is worse than an acknowledged gap, because a reader cannot tell the
two apart afterwards.

The sections are declared in `policy.yaml` under `session.record_sections`, with the same
contract the handover sections have: a section named there with no writer in `lib/derive.sh`
is a configuration error reported by name, never a silently empty heading.

**The body is scoped to the episode, not to the task.** A task outlives the episodes that
work on it. Deriving the commit range from the task's opening commit made the first draft of
this section claim 1261 commits and 2339 changed files for an episode that had made one and
forty-nine. An episode's record is scoped by the same `start_head` its own front matter
records.

**The lifecycle commits the record it writes.** `capture session --event end` commits the
published record, and three properties are not negotiable:

- *It commits that path and nothing else.* The tree is composed from `HEAD` plus the one
  record path in a temporary index, so whatever the worker had staged when the window closed
  stays staged and uncommitted.
- *It runs no commit hook.* `.githooks/pre-commit` runs `doctor`, the worktree guard and the
  pages freshness check — together far longer than a provider allows an end hook, which
  would kill the commit halfway. The commit is therefore made with plumbing
  (`read-tree`/`write-tree`/`commit-tree`/`update-ref`), which has no hooks to run because
  no porcelain runs.
- *It never fails.* Every path returns 0. A record that could not be committed is exactly as
  committed as it was before, and `doctor` still names it.

**The push obeys the push contract.** The record is pushed only as a fast-forward this
function verified itself, through `.githooks/pre-push` like any other push. That hook asks
`majordomus finish --check` — whether the *task* may be published — and for most of a task's
life the answer is no, so the push often will not go out. That is the correct outcome and
not a limitation to engineer around: the record is already committed and travels with the
worker's next push. Bypassing the hook would mean this path, alone in the tool, decides that
the repository's own push contract does not apply to it.

# Failure behaviour

`test/cases/63_session_records.sh` closes an episode with no authored summary and refuses a
record whose body is empty, whose commit range is the task's rather than the episode's, or
which the close left uncommitted. A section declared in `session.record_sections` with no
writer ends `session close` with exit 10 naming the section.

# Verification

    majordomus session close            # with no stdin: the record has a body
    test/cases/63_session_records.sh    # the body, its scope, and the commit
