# Continuity — how work survives the session that was doing it

A conversation with an AI worker is not a place to keep anything. It ends, it is
compacted, it is replaced by a different worker with a different context window, and
none of that is under your control. The work, meanwhile, has to continue.

Majordomus's answer is that operational state lives in the repository, in files that
outlive every conversation, and that the next worker is given a briefing assembled from
those files rather than a transcript to re-read.

```
Conversation is ephemeral.
Operational state is durable.
Context is selected.
History is reconstructable.
Handovers are explicit.
Completion is independently verified.
```

## The problem, stated precisely

A worker stops. Something must survive, or the next worker begins by reconstructing what
the last one knew — from commit messages, from a diff, from a human's memory of a
conversation they were not part of. That reconstruction is slow, and worse, it is
confidently wrong often enough to matter.

The obvious fix is to save the conversation. That fix is wrong for three reasons.

A transcript is not state. It is a record of how someone arrived at a conclusion,
including every wrong turn, in an order optimised for nothing. What the next worker needs
is the conclusion, the current facts, and the next action.

A transcript is unbounded. It grows with the length of the session rather than with the
size of the work, and the tenth session's context is filled with the first session's
false starts.

A transcript goes stale silently. It records what was true when it was written and says
nothing when the repository moves past it. A worker that trusts a stale narrative makes
decisions against a repository that no longer exists.

So Majordomus stores none of it. What it stores is small, typed, and each piece has one
home.

## Six kinds of durable record, and a seventh that is not task-shaped

| Record | Answers | Mutability | Where |
|---|---|---|---|
| **task** | what is being worked on, under what constraints, within which paths | mutable; one active per checkout | `state/current.yaml` |
| **checkpoint** | what was true a few minutes ago, and what comes next | append-only | `state/checkpoints/` |
| **handover** | the deliberate package for whoever continues | append-only | `state/handovers/` |
| **decision** | what was decided, why, and what was rejected | append-only | `state/decisions.md` |
| **open question** | what is unresolved, and what it blocks | mutable index; the ledger keeps the history | `state/open-questions.md` |
| **history event** | what happened, when, for which task, at which commit | append-only | `state/ledger.jsonl` |

Prompt assets in `.ai/repo/prompts/` are a seventh thing, but they are not records of
work — they are reusable framings, versioned with the repository.

## The seventh kind: the session

The six above are all task-shaped. Each one hangs off the work, which is right, and it
leaves one question with nowhere to live: *what did one worker do between sitting down and
stopping?*

That question is not the same as any of the six. A task can outlive a worker; a worker can
touch three tasks in an afternoon. Nothing in the six records that two decisions an hour
apart, filed under different tasks, were made by the same person in the same sitting — and
that is exactly the causal thread a later reader is trying to pick up.

| Record | Answers | Mutability | Where |
|---|---|---|---|
| **session** | what one execution episode did, between which commits, producing which records | one open per worktree, then immutable | `state/session-current.yaml`, then `state/sessions/` |

A session opens, may cross several tasks, and closes. `task != session` in both
directions: a task spanning two sessions is named by both, and a session spanning two
tasks names both.

### A session is an envelope, not a narrative

The closed record is identity, a temporal boundary, and references. It names the tasks,
issues, milestones, checkpoints, handovers, decisions, questions and evidence of its
episode, and it copies the body of none of them. An authored summary is allowed and is
not authority; the records it points at are.

The temptation is to write the episode down instead — one document with the decisions
quoted, the checkpoint bodies inlined, the git log pasted, the reasoning narrated. That
document is a transcript with better formatting, and it fails the same three ways any
transcript does: it is a record of how somebody arrived somewhere rather than where they
are, it grows with the length of the session rather than the size of the work, and it goes
stale silently while the repository moves past it.

So the session file holds pointers, and the pointers are checked. A reference to a
checkpoint that does not exist is a reported defect, not a broken link nobody notices.

### The envelope is derived, not accumulated

Nothing writes into an open session. `checkpoint`, `decision`, `question` and `plan` are
unchanged and know nothing about sessions. At close, the reference lists are computed from
the ledger.

The ledger is already append-only, already written only by Majordomus, already ordered by
the order the commands ran, and already validated. It knows every fact the envelope needs.
Having each command additionally append to a session file would put a write on the hot path
of commands that today append one line, and would create a second mutable account of
events the ledger already holds — which is the second source of truth this whole design
refuses. The cost is that the ledger is now load-bearing for two purposes, and that is
stated rather than hidden.

What it selects, and why it is not a time range, is worth stating because the obvious
implementation is wrong. Every ledger line now carries the session that wrote it, next to
the commit and branch it already carried. A first version selected every line between the
session's two timestamps instead, and the first time it ran for real it claimed another
worker's tasks, checkpoints and handovers: the ledger is one file per repository, two
workers were writing to it, and no timestamp can separate them. Which episode wrote an
event is something the machine knows when it writes it, so it is recorded then.

A line with no session belongs to no episode. Sessions are optional, and work done outside
one is attributed to nobody rather than to whoever had a session open nearby.

### Sessions are read with the same rules as everything else

Resolution is the two-tier rule: same repository, same worktree, same branch; then same
repository, same branch; then nothing. A session from another branch is never offered.

Divergence uses the same four labels — `exact`, `advanced`, `diverged`,
`different_context` — computed at read time from the commit the session closed at. A
session whose branch was rewritten underneath it is labelled `diverged` wherever it is
printed, and is never quietly presented as current knowledge.

Ordering comes from the recorded timestamp, with ledger line order breaking ties inside
one second. Nothing reads filesystem modification time: it does not survive a clone, and
it is not the time the record asserts.

### What a session is not

It is not a scope. A task claims paths; an episode does not, and giving one paths would
create a second, weaker claim for the coordination check to disagree with.

It is not a unit of acceptance. `finish` evaluates a task against its contract. Closing a
session asserts only that the episode ended.

It is not required. A worker that never opens a session loses the episode boundary and
nothing else; every other record is written exactly as before.

## Checkpoint and handover are different objects

They are easy to conflate and expensive to conflate.

A **checkpoint** is a progress note inside an active task. It is written often, it is
short by policy (`checkpoint.max_body_lines`), and its job is to be quotable whole into
the next briefing. "Reproduced the fault with the fixture; the cause is in normalisation,
not comparison; next, write the regression test."

A **handover** is a deliberate continuation package written when a worker stops. It has
required sections, it is refused if any of them is empty, and it is the thing another
worker resumes from. It is written rarely.

A checkpoint that grows into a report is refused rather than truncated, with the
suggestion to write a handover instead. The cap is the mechanism that keeps the two
objects distinct.

## Identity comes from reality

`repository_id`, `worktree`, `branch`, `head`, `working_tree` and `changed_files` on every
record are computed from git at write time. A body that contains any of them is refused.

This is not fussiness. A worker asked to summarise its own state will produce a plausible
summary, and a plausible summary that includes a commit hash the worker did not verify is
worse than no summary, because it looks checkable. The fields a machine can determine are
determined by the machine; the fields only a person or a worker can supply are supplied by
them and marked as such.

## Records are read with a divergence label

Nothing is trusted because it exists. On read, the recorded `head` is compared with the
current `HEAD` through `git merge-base`, and the record is labelled:

| Label | Meaning | What to do |
|---|---|---|
| `exact` | written at this commit | trust it |
| `advanced` | git has moved forward since | trust it, expect some of it to be done |
| `diverged` | the recorded commit is not an ancestor | history was rewritten; trust git, not the record |
| `different_context` | another branch | this record is not about your work |

`context` prints the label. `check` and `watch` report it as a finding. A record is
evidence, never authority.

## Resolution is deterministic, and absence is a valid answer

`handover --resolve`, `checkpoint --show` and `context` all use the same rule:

1. Same repository, same worktree, same branch.
2. Same repository, same branch, when the branch is not detached.
3. Nothing.

There is no third tier. A record from an unrelated worktree or branch is never offered,
because a globally newer record silently becoming your context is worse than having no
context at all — you cannot tell that it is wrong until you have acted on it.

Records written inside the same second would otherwise resolve in an order decided by a
random filename suffix, so the ledger, which is append-only and written in command order,
breaks the tie.

When nothing matches, the answer is "no relevant handover", not the closest thing
available. **Absence is better than incorrect memory.**

## Context is selected, not accumulated

`majordomus context` assembles a briefing in authority order — git, then the task and its
profile, then blockers, then authored records, then event history — and prints it. Nothing
is persisted; it is recomputed each time from the records and git.

What goes in is decided by the profile's `context` block. A `routine` task does not need
the decision history; a `deep-work` task wants the repository's decisions rather than only
its own. Those toggles were configuration that nothing read until the builder existed;
now they are the thing that decides.

When the assembled text exceeds `context.builder_budget_lines`, sections are dropped in a
fixed order: history first, then touched files, then decisions, then the bodies of the
checkpoint and the handover, which degrade to a pointer rather than disappearing. Git, the
task, the profile and open blockers are never dropped.

**Every exclusion is named, with its reason**, under `EXCLUDED`. An under-filled context is
debugged by reading that list, not by guessing what the tool decided to leave out. The
reported line count is the count of the document actually printed, including its own
header and trailer.

## History is for reconstruction, not for reading back a conversation

`state/ledger.jsonl` is append-only and written only by Majordomus. One line per event:
started, checkpoint, decision recorded, question opened, question resolved, handed over,
finished, projections updated, ledger rotated. Each carries a timestamp, the task, and the
commit it happened at.

`majordomus history` reads it back with filters. It answers what happened, when, for which
task, at what git state, what verification ran and what outcome was accepted. It does not
answer what anyone said, because that is not recorded.

Retention is deliberate: the ledger has a line cap, and `history --rotate` moves the
oldest lines into a dated archive file. It never deletes, and it refuses to overwrite an
existing archive.

## Blockers are state, not prose

An unresolved question in a handover paragraph is a note. An entry in
`state/open-questions.md` **refuses `finish --outcome completed`** — any entry, not only one
the active task opened. A question is a person owing an answer, and the work it blocks
outlives the task that asked; a gate that forgot at the task boundary would be a gate a
handover walks past. The store is checkout-local, under the ignored `.ai/local/state/`,
so the questions a gate can see are the ones asked in this working copy; a question is
carried to another checkout by the handover that names it, never by git.

Only *completed* is refused. `blocked`, `partial`, `no_match` and `failed` are honest
statements that the work did not finish, and refusing them would buy a green gate by forcing
a mislabelled outcome.

That gate is why the file has a machine-written line format, and why `check`, `doctor` and
`watch` fail on an entry that does not parse: a gate that cannot read an entry is a gate
that can be bypassed by mistyping one.

## Nothing here calls a model

Majordomus stores, validates, resolves, projects and verifies. It does not summarise, it
does not decide what a checkpoint should say, and it makes no network call. If you want a
model to write your checkpoint, have the worker write it and pipe it in. The tool is
deterministic infrastructure and stays that way.

## Where the lifecycle puts each piece

```
majordomus start "<task>" --scope <paths> [--profile <name>]
        |                                  names any prior record for this branch
        v
majordomus context                         the briefing, within budget
        |
        v
   work happens (Majordomus is not involved)
        |
        +---> majordomus checkpoint        progress, often, short
        +---> majordomus decision add      what was decided and why
        +---> majordomus question add      what is unresolved, and it now blocks
        |
        v
majordomus check                           scope, state, blockers, store integrity
        |
        +-- not finished --> majordomus handover   ---> the next session runs context
        |
        v
majordomus finish --outcome <...> --verify-command "<cmd>"
        |
        v
majordomus history                         the lifecycle, reconstructable
```

## What this does not do

It does not know who wrote which commit. A task records the commit it started at, and
`check` treats every file changed since as the task's work. When another session commits
to the same branch in the same checkout, that session's files are reported outside your
scope. The report is correct about the files and wrong about the author, and Majordomus
has no way to tell the difference. Close the task with a handover and start a new one at
the current commit.

**A question blocks its whole branch, not the task that asked.** That is deliberate, and it
is wrong in one case: a question about work that was abandoned, left unanswered, refuses
completion of every later task on that branch until somebody writes an answer. The direction
was chosen because it fails loudly — `question list` names it and `question resolve` clears
it in one command — where the alternatives fail silently. See `M001` in
`.ai/repo/project/` for the alternatives that were rejected and the case each gets wrong.

It does not measure tokens, context savings, or cost. The budget is lines, because lines
are what it can count. See [`ECONOMICS.md`](ECONOMICS.md).

It does not rank search results, embed anything, or maintain an index. `search` is a
literal grep across the record kinds in authority order. The corpus is a handful of files;
an index would be a second source of truth that has to be kept in step with the first.

It does not resolve across worktrees or branches, and it never will silently.

## Related

[`CONCEPTS.md`](CONCEPTS.md) for the vocabulary · [`CLI.md`](CLI.md) for every command ·
[`SCHEMAS.md`](SCHEMAS.md) for the file formats · [`DESIGN.md`](DESIGN.md) for why the
models are shaped this way.
